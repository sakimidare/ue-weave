//! 单篇文章的读取、解析、资源处理、导出和结果汇报。
use crate::{
    cli::Args,
    diagnostic::Diagnostic,
    downloader,
    error::{RecoverError, Result},
    explain, export, input,
    log::Logger,
    model::Ir,
    network, network_policy, parser, profile, regenerate, site, site_config,
};
use reqwest::blocking::Client;
use scraper::Html;
use std::{fs, path::Path, time::Duration};

struct Network {
    client: Client,
    policy: network::Policy,
}

impl Network {
    fn new(args: &Args) -> Result<Self> {
        let client = network_policy::client(
            Duration::from_secs(args.timeout),
            args.allow_private_network,
        )
        .map_err(|source| RecoverError::Request {
            url: args.url.clone().unwrap_or_default(),
            source,
        })?;
        Ok(Self {
            client,
            policy: network::Policy {
                retries: args.retries,
                allow_private_network: args.allow_private_network,
                ..Default::default()
            },
        })
    }
}

enum Source<'a> {
    File(&'a Path),
    Url(&'a str),
}

impl<'a> Source<'a> {
    fn from_args(args: &'a Args) -> Result<Self> {
        match (args.input.as_deref(), args.url.as_deref()) {
            (Some(path), None) => Ok(Self::File(path)),
            (None, Some(url)) => Ok(Self::Url(url)),
            _ => Err(RecoverError::Arguments("输入 URL 与 --input 二选一".into())),
        }
    }

    fn load(&self, network: Option<&Network>, logger: &Logger) -> Result<input::Input> {
        match self {
            Self::File(path) => fs::read(path)
                .map(|bytes| input::Input::decode(bytes, path.display().to_string(), None))
                .map_err(|source| RecoverError::ReadInput {
                    path: (*path).into(),
                    source,
                }),
            Self::Url(url) => {
                let network = network
                    .ok_or_else(|| RecoverError::Arguments("URL 输入需要网络客户端".into()))?;
                let download = network::fetch(
                    &network.client,
                    url,
                    network.policy,
                    Some(network::MAX_BODY_SIZE),
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
        }
    }
}

struct Parsed {
    ir: Ir,
    source: String,
    selector: String,
    requested_selector: String,
}

fn parse(input: input::Input, args: &Args, logger: &Logger) -> Result<Parsed> {
    let mut settings = args
        .site_config
        .as_deref()
        .map(site_config::Selectors::load)
        .transpose()?
        .unwrap_or_default();
    logger.stage(
        "input",
        format!(
            "读取完成，{} 字节，来源 {}",
            input.bytes.len(),
            input.source
        ),
    );
    let document = Html::parse_document(&input.html);
    let requested_selector = settings
        .apply_overrides(&args.selector)?
        .unwrap_or_else(|| "auto".into());
    let selector =
        site::content_selector(&document, settings.content.as_deref().unwrap_or("auto"))?;
    logger.stage("parse", format!("使用正文选择器 {selector}"));
    let mut ir = parser::parse(
        &document,
        &selector,
        args.no_inference,
        Some(input.source.clone()),
    )?;
    settings.apply_metadata(&document, &mut ir.metadata);
    ir.warnings.extend(input.warnings);
    Ok(Parsed {
        ir,
        source: input.source,
        selector,
        requested_selector,
    })
}

#[derive(Default)]
struct AssetErrors {
    attachments: Vec<Diagnostic>,
    images: Vec<Diagnostic>,
}

fn process_assets(
    ir: &mut Ir,
    args: &Args,
    output: &Path,
    network: Option<&Network>,
    logger: &Logger,
) -> Result<AssetErrors> {
    let Some(network) = network else {
        return Ok(AssetErrors::default());
    };
    let attachments = if args.download_attachments {
        downloader::download_attachments(ir, output, &network.client, network.policy, logger)?
    } else {
        Vec::new()
    };
    let images = if args.download_images {
        downloader::download_images(ir, output, &network.client, network.policy, logger)?
    } else {
        Vec::new()
    };
    ir.warnings
        .extend(attachments.iter().chain(&images).cloned());
    Ok(AssetErrors {
        attachments,
        images,
    })
}

fn write_files(path: &Path, files: Vec<(String, Vec<u8>)>) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| RecoverError::CreateOutput {
        path: path.into(),
        source,
    })?;
    for (name, contents) in files {
        let path = path.join(name);
        fs::write(&path, contents).map_err(|source| RecoverError::WriteOutput { path, source })?;
    }
    Ok(())
}

fn report(
    parsed: &Parsed,
    errors: &AssetErrors,
    rendered: &profile::Output,
    args: &Args,
    logger: &Logger,
) -> Result<()> {
    let ir = &parsed.ir;
    logger.warnings(&ir.warnings, &ir.original_nodes, &rendered.ir);
    logger.done(
        ir.blocks.iter().map(crate::model::Block::block_count).sum(),
        ir.attachments.len(),
        ir.images.len(),
    );
    if args.json {
        let result = serde_json::json!({
            "status": "ok", "source": parsed.source, "selector": parsed.selector,
            "requested_selector": parsed.requested_selector, "warnings": ir.warnings,
            "output_profile": args.profile, "output_options": args.output_options.resolve(args.profile),
            "attachment_errors": errors.attachments, "image_errors": errors.images,
            "content_integrity": ir.metadata.get("content_integrity"),
            "resource_integrity": ir.metadata.get("resource_integrity"),
            "output_changes": rendered.changes,
            "counts": {"blocks": ir.blocks.len(), "attachments": ir.attachments.len(), "images": ir.images.len()}
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|source| RecoverError::Serialize {
                file: "stdout".into(),
                source
            })?
        );
    }
    logger.finish_diagnostics(&ir.warnings);
    Ok(())
}

pub(crate) fn run(args: &Args) -> Result<()> {
    if let Some(code) = &args.explain {
        return explain::print(code, args.color);
    }
    let output = args
        .output
        .as_deref()
        .ok_or_else(|| RecoverError::Arguments("请用 --output 指定输出目录".into()))?;
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
        return regenerate::run(path, args, output);
    }
    let source = Source::from_args(args)?;
    let logger = Logger::with_color(args.verbose, args.color);
    let network =
        (matches!(source, Source::Url(_)) || args.download_images || args.download_attachments)
            .then(|| Network::new(args))
            .transpose()?;
    logger.stage("input", "开始读取输入");
    let mut parsed = parse(source.load(network.as_ref(), &logger)?, args, &logger)?;
    let errors = process_assets(&mut parsed.ir, args, output, network.as_ref(), &logger)?;
    let settings = args.output_options.resolve(args.profile);
    let rendered = if args.json {
        profile::prepare(&parsed.ir, settings)
    } else {
        profile::prepare_documents(&parsed.ir, settings)
    };
    let documents = export::render(&rendered.ir, settings, &args.exports);
    write_files(output, args.exports.files(documents, &parsed.ir)?)?;
    report(&parsed, &errors, &rendered, args, &logger)
}
