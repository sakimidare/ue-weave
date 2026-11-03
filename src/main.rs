#[cfg(test)]
mod acceptance;
mod asset_format;
mod asset_name;
mod attachment;
mod audit;
mod css;
mod downloader;
mod error;
mod export;
mod inference;
mod inline;
mod input;
mod integrity;
mod ir_format;
mod log;
mod markdown;
mod model;
mod network;
mod network_policy;
#[cfg(test)]
mod network_tests;
mod parser;
mod profile;
mod regenerate;
mod renderer;
#[cfg(test)]
mod robustness;
mod rules;
mod safety;
mod site;
mod site_config;
mod snapshot;
mod style_registry;
mod table;
mod typst;
mod word;

use clap::Parser;
use error::{RecoverError, Result};
use log::Logger;
use scraper::Html;
use std::time::Duration;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "ue-weave",
    version,
    about = "提取网页文章，导出 HTML、Markdown、Typst 或 JSON",
    after_help = "URL、--input、--input-ir 三选一。格式开关用 =false 关闭，细分开关优先于组合开关和预设。\n\n示例：\n  ue-weave https://example.org/article.html -o output/\n  ue-weave --input page.html -o output/ --formats markdown --profile aggressive"
)]
struct Args {
    /// 文章网址（HTTP/HTTPS）。
    url: Option<String>,
    /// 读取本地 HTML 文件。
    #[arg(long, value_name = "FILE", help_heading = "输入与正文")]
    input: Option<PathBuf>,
    /// 从 JSON 正文生成文档。
    #[arg(long, value_name = "FILE", help_heading = "输入与正文")]
    input_ir: Option<PathBuf>,
    /// JSON 的资源目录，默认使用 JSON 所在目录。
    #[arg(
        long,
        requires = "input_ir",
        value_name = "DIR",
        help_heading = "资源与网络"
    )]
    asset_root: Option<PathBuf>,
    /// 正文或字段选择器，可重复；如 '.article-body'、'title=h1'。
    #[arg(long, value_name = "[FIELD=]CSS", help_heading = "输入与正文")]
    selector: Vec<String>,
    /// 读取选择器 JSON 配置。
    #[arg(long, value_name = "FILE", help_heading = "输入与正文")]
    site_config: Option<PathBuf>,
    /// 重试次数，0–5；0 关闭。
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=5), value_name = "N", help_heading = "资源与网络")]
    retries: u8,
    /// 单次请求超时秒数，1–120。
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120), value_name = "SECONDS", help_heading = "资源与网络")]
    timeout: u64,
    /// 允许请求私网、回环和保留网段。
    #[arg(long, help_heading = "资源与网络")]
    allow_private_network: bool,
    /// 输出目录，自动创建；覆盖同名文档。
    #[arg(short, long, value_name = "DIR", help_heading = "输出")]
    output: PathBuf,
    /// 关闭标题、署名和缩进推断。
    #[arg(long, help_heading = "输入与正文")]
    no_inference: bool,
    /// 显示详细日志。
    #[arg(long, help_heading = "运行信息")]
    verbose: bool,
    /// 在终端输出 JSON 运行结果；正文用 --formats json。
    #[arg(long, help_heading = "运行信息")]
    json: bool,
    /// 下载图片到 images/，改用本地链接。
    #[arg(long, help_heading = "资源与网络")]
    download_images: bool,
    /// 下载附件到 attachments/，改用本地链接。
    #[arg(long, help_heading = "资源与网络")]
    download_attachments: bool,
    /// 选择 HTML、Markdown 和 Typst 的格式预设。
    #[arg(long, value_enum, default_value_t = profile::Profile::Conservative, help_heading = "输出")]
    profile: profile::Profile,
    #[command(flatten)]
    output_options: profile::Overrides,
    #[command(flatten)]
    exports: export::Options,
}

fn load_input(args: &Args, logger: &Logger) -> Result<input::Input> {
    if args.url.is_some() == args.input.is_some() {
        return Err(RecoverError::Arguments("输入 URL 与 --input 二选一".into()));
    }
    if let Some(path) = &args.input {
        return fs::read(path)
            .map(|bytes| input::Input::decode(bytes, path.display().to_string(), None))
            .map_err(|source| RecoverError::ReadInput {
                path: path.clone(),
                source,
            });
    }
    let url = args.url.clone().expect("validated URL");
    let client = network_policy::client(
        Duration::from_secs(args.timeout),
        args.allow_private_network,
    )
    .map_err(|source| RecoverError::Request {
        url: url.clone(),
        source,
    })?;
    let download = network::fetch(
        &client,
        &url,
        network::Policy {
            retries: args.retries,
            allow_private_network: args.allow_private_network,
            ..Default::default()
        },
        Some(64 * 1024 * 1024),
        logger,
    )?;
    logger.stage(
        "request",
        format!("请求完成，尝试 {} 次", download.attempts),
    );
    Ok(input::Input::decode(
        download.bytes,
        download.url,
        download.charset,
    ))
}

fn run(args: &Args) -> Result<()> {
    if let Some(path) = &args.input_ir {
        if args.url.is_some()
            || args.input.is_some()
            || args.download_images
            || args.download_attachments
            || args.site_config.is_some()
            || !args.selector.is_empty()
        {
            return Err(RecoverError::Arguments(
                "请为 --input-ir 使用独立导出命令；网页输入、下载和选择器选项用于网页解析命令"
                    .into(),
            ));
        }
        return regenerate::run(path, args);
    }
    let logger = Logger::new(args.verbose);
    let mut site_settings = args
        .site_config
        .as_deref()
        .map(site_config::Selectors::load)
        .transpose()?
        .unwrap_or_default();
    logger.stage("input", "开始读取输入");
    let input::Input {
        bytes,
        html,
        source,
        warnings,
    } = load_input(args, &logger)?;
    fs::create_dir_all(&args.output).map_err(|source| RecoverError::CreateOutput {
        path: args.output.clone(),
        source,
    })?;
    logger.stage(
        "input",
        format!("读取完成，{} 字节，来源 {}", bytes.len(), source),
    );
    let document = Html::parse_document(&html);
    let requested_selector = site_settings.apply_overrides(&args.selector)?;
    let requested = site_settings.content.as_deref().unwrap_or("auto");
    let selected = site::content_selector(&document, requested)?;
    logger.stage("parse", format!("使用正文选择器 {}", selected));
    let parsed_ir = parser::parse(
        &document,
        &selected,
        args.no_inference,
        Some(source.clone()),
    )?;
    let mut ir = parsed_ir;
    site_settings.apply_metadata(&document, &mut ir.metadata);
    ir.warnings.extend(warnings);
    let (attachment_errors, image_errors) = if args.download_attachments || args.download_images {
        let client = network_policy::client(
            Duration::from_secs(args.timeout),
            args.allow_private_network,
        )
        .map_err(|source| RecoverError::Request {
            url: source.to_string(),
            source,
        })?;
        let attachments = if args.download_attachments {
            downloader::download_attachments(
                &mut ir,
                &args.output,
                &client,
                network::Policy {
                    retries: args.retries,
                    allow_private_network: args.allow_private_network,
                    ..Default::default()
                },
                &logger,
            )?
        } else {
            Vec::new()
        };
        let images = if args.download_images {
            downloader::download_images(
                &mut ir,
                &args.output,
                &client,
                network::Policy {
                    retries: args.retries,
                    allow_private_network: args.allow_private_network,
                    ..Default::default()
                },
                &logger,
            )?
        } else {
            Vec::new()
        };
        (attachments, images)
    } else {
        (Vec::new(), Vec::new())
    };
    ir.warnings
        .extend(attachment_errors.iter().chain(&image_errors).cloned());
    let settings = args.output_options.resolve(args.profile);
    let output = if args.json {
        profile::prepare(&ir, settings)
    } else {
        profile::prepare_documents(&ir, settings)
    };
    let documents = export::render(&output.ir, settings, &args.exports);
    for (name, contents) in args.exports.files(documents, &ir)? {
        let path = args.output.join(name);
        fs::write(&path, contents).map_err(|source| RecoverError::WriteOutput { path, source })?;
    }
    logger.warnings(&ir.warnings);
    logger.done(
        ir.blocks.iter().map(model::Block::block_count).sum(),
        ir.attachments.len(),
        ir.images.len(),
    );
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok", "source": source, "selector": selected,
                "requested_selector": requested_selector.unwrap_or_else(|| "auto".into()), "warnings": ir.warnings,
                "output_profile": args.profile, "output_options": settings,
                "attachment_errors": attachment_errors, "image_errors": image_errors,
                "output_changes": output.changes,
                "counts": {"blocks": ir.blocks.len(), "attachments": ir.attachments.len(), "images": ir.images.len()}
            })).map_err(|source| RecoverError::Serialize {
                file: "stdout".into(),
                source
            })?
        );
    }
    Ok(())
}

fn main() {
    let args = Args::parse();
    if let Err(error) = run(&args) {
        if args.json {
            println!(
                "{}",
                serde_json::json!({"status":"error", "code":error.code(), "error":error.to_string(), "network":match &error { RecoverError::Network(error) => error.details(), _ => serde_json::Value::Null }})
            );
        }
        eprintln!("ue-weave: error: {error}");
        std::process::exit(1);
    }
}
