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
#[command(name = "ue-weave", about = "UEditor/WebPlus HTML 反编译与结构还原器")]
struct Args {
    url: Option<String>,
    #[arg(long)]
    input: Option<PathBuf>,
    /// 从 JSON 正文生成文档。
    #[arg(long)]
    input_ir: Option<PathBuf>,
    /// IR 资源根目录，包含 images/ 和 attachments/。
    #[arg(long, requires = "input_ir")]
    asset_root: Option<PathBuf>,
    /// CSS 或 字段=CSS，可重复；字段为 content/title/date/source/author/publisher。
    #[arg(long, value_name = "[FIELD=]CSS")]
    selector: Vec<String>,
    /// 站点选择器 JSON 配置。
    #[arg(long)]
    site_config: Option<PathBuf>,
    /// 临时网络故障的重试次数。
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=5))]
    retries: u8,
    /// 每次请求的总超时，单位秒。
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120))]
    timeout: u64,
    /// 允许请求私网、回环和保留网段。
    #[arg(long)]
    allow_private_network: bool,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    no_inference: bool,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    download_images: bool,
    #[arg(long)]
    download_attachments: bool,
    /// 选择 HTML、Markdown 和 Typst 的格式预设。
    #[arg(long, value_enum, default_value_t = profile::Profile::Conservative)]
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
