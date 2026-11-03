#[cfg(test)]
mod acceptance;
mod attachment;
mod audit;
mod css;
mod downloader;
mod error;
mod inference;
mod inline;
mod input;
mod integrity;
mod log;
mod markdown;
mod model;
mod network;
#[cfg(test)]
mod network_tests;
mod parser;
mod profile;
mod raw_html;
mod regenerate;
mod renderer;
mod rules;
mod safety;
mod site;
mod site_config;
mod snapshot;
mod table;
mod typst;
mod word;

use clap::Parser;
use error::{RecoverError, Result};
use log::Logger;
use scraper::{Html, Selector};
use std::time::Duration;
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "ue-recover", about = "UEditor/WebPlus HTML 反编译与结构还原器")]
struct Args {
    url: Option<String>,
    #[arg(long)]
    input: Option<PathBuf>,
    /// 从已有输出目录中的编辑后 IR 重新生成文档。
    #[arg(long)]
    input_ir: Option<PathBuf>,
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
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    no_inference: bool,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    json: bool,
    /// 额外保存解码后的完整输入页面 source.html；original.html 始终输出。
    #[arg(long)]
    keep_raw: bool,
    #[arg(long)]
    download_images: bool,
    #[arg(long)]
    download_attachments: bool,
    /// 输出格式保留程度；原始快照和规范 IR 不受影响。
    #[arg(long, value_enum, default_value_t = profile::Profile::Conservative)]
    profile: profile::Profile,
    #[command(flatten)]
    output_options: profile::Overrides,
}

fn load_input(args: &Args, logger: &Logger) -> Result<input::Input> {
    if args.url.is_some() == args.input.is_some() {
        return Err(RecoverError::Arguments(
            "必须提供 URL 或 --input，且只能提供一个".into(),
        ));
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
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(args.timeout))
        .user_agent("ue-recover/0.1")
        .build()
        .map_err(|source| RecoverError::Request {
            url: url.clone(),
            source,
        })?;
    let download = network::fetch(
        &client,
        &url,
        network::Policy {
            retries: args.retries,
            ..Default::default()
        },
        None,
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
                "--input-ir 与网页输入、资源下载、站点配置选项不能同时使用".into(),
            ));
        }
        return regenerate::run(
            path,
            &args.output,
            args.json,
            args.verbose,
            args.profile,
            args.output_options.resolve(args.profile),
        );
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
    let path = args.output.join("source.bin");
    fs::write(&path, &bytes).map_err(|source| RecoverError::WriteOutput { path, source })?;
    logger.stage(
        "input",
        format!("读取完成，{} bytes，来源 {}", html.len(), source),
    );
    let document = Html::parse_document(&html);
    let requested_selector = site_settings.apply_overrides(&args.selector)?;
    let requested = site_settings.content.as_deref().unwrap_or("auto");
    let selected = site::content_selector(&document, requested)?;
    let selector = Selector::parse(&selected).expect("validated selector");
    logger.stage("parse", format!("使用正文选择器 {}", selected));
    let parsed_ir = parser::parse(
        &document,
        &selected,
        args.no_inference,
        Some(source.clone()),
    )?;
    let root = document
        .select(&selector)
        .next()
        .ok_or_else(|| RecoverError::ContentNotFound {
            selector: selected.clone(),
        })?;
    let mut ir = parsed_ir;
    site_settings.apply_metadata(&document, &mut ir.metadata);
    ir.warnings.extend(warnings);
    let (attachment_errors, image_errors) = if args.download_attachments || args.download_images {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(args.timeout))
            .user_agent("ue-recover/0.1 attachment-downloader")
            .build()
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
    let original = raw_html::extract(&html, root).unwrap_or_else(|| {
        ir.warnings.push(serde_json::json!({"kind":"raw-snapshot-fallback","value":"normalized-dom","confidence":"unknown","reason":"无法唯一校验正文源码范围；original.html 使用 DOM 序列化，完整原始字节保存在 source.bin","source_nodes":[]}));
        root.html()
    });
    let reconstructed_json =
        serde_json::to_string_pretty(&ir).map_err(|source| RecoverError::Serialize {
            file: "reconstructed.json".into(),
            source,
        })?;
    let settings = args.output_options.resolve(args.profile);
    let output = profile::prepare(&ir, settings);
    let reconstructed_html = renderer::render(&output.ir);
    let reconstructed_markdown = if settings.native_markdown {
        markdown::render_editable(&output.ir)
    } else {
        markdown::render(&output.ir)
    };
    let reconstructed_typst = typst::render(&output.ir);
    let report = serde_json::json!({"source":source,"selector":args.selector,"inference_disabled":args.no_inference,"download_attachments":args.download_attachments,"download_images":args.download_images,"warnings":ir.warnings,"attachment_errors":attachment_errors,"image_errors":image_errors,"counts":{"blocks":ir.blocks.len(),"total_blocks":ir.blocks.iter().map(model::Block::block_count).sum::<usize>(),"attachments":ir.attachments.len(),"images":ir.images.len(),"original_nodes":ir.original_nodes.len()},"limitations":["这是反编译/结构还原，目标是生成可编辑结构模型。","UEditor 保存的是 HTML，系统没有可供完整逆向的专有二进制格式。","filterWord 删除的信息无法恢复。","original.html 始终保留。"]});
    let mut report = report;
    report["selector"] = serde_json::json!(selected);
    report["requested_selector"] =
        serde_json::json!(requested_selector.unwrap_or_else(|| "auto".into()));
    report["selector_overrides"] = serde_json::json!(args.selector);
    report["site_selectors"] = serde_json::json!(site_settings);
    report["network"] = serde_json::json!({"retries":args.retries,"timeout_seconds":args.timeout});
    report["output_profile"] = serde_json::json!(args.profile);
    report["output_options"] = serde_json::json!(settings);
    report["output_changes"] = serde_json::json!(output.changes);
    let report_json =
        serde_json::to_string_pretty(&report).map_err(|source| RecoverError::Serialize {
            file: "report.json".into(),
            source,
        })?;
    logger.stage("output", format!("写入输出目录 {:?}", args.output));
    fs::create_dir_all(&args.output).map_err(|source| RecoverError::CreateOutput {
        path: args.output.clone(),
        source,
    })?;
    if args.keep_raw {
        let path = args.output.join("source.html");
        fs::write(&path, &html).map_err(|source| RecoverError::WriteOutput { path, source })?;
    }
    for (name, contents) in [
        ("original.html", original),
        ("reconstructed.json", reconstructed_json),
        ("reconstructed.html", reconstructed_html),
        ("reconstructed.md", reconstructed_markdown),
        ("reconstructed.typ", reconstructed_typst),
        ("report.json", report_json),
    ] {
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
            serde_json::to_string_pretty(&report).map_err(|source| RecoverError::Serialize {
                file: "report.json".into(),
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
        eprintln!("ue-recover: error: {error}");
        std::process::exit(1);
    }
}
