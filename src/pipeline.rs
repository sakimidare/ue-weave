//! 单篇文章的读取、解析、资源处理、导出和结果汇报。
use crate::network_policy::Client;
use crate::{
    cli::Args,
    downloader,
    error::{RecoverError, Result},
    explain, export, input,
    log::Logger,
    model::Ir,
    network, network_policy, parser, profile, regenerate, site, site_config,
};
use reqwest::header::HeaderMap;
use std::{fs, path::Path, time::Duration};

struct Network {
    client: Client,
    policy: network::Policy,
}

impl Network {
    fn new(args: &Args) -> Result<Self> {
        let headers = load_headers(args)?;
        if !args.allow_private_network
            && args
                .resolve
                .iter()
                .any(|(_, ip)| !network_policy::public_ip(*ip))
        {
            return Err(RecoverError::Arguments(
                "固定地址属于校内或保留网段，请添加 --allow-private-network".into(),
            ));
        }
        let mut client = network_policy::client_with_addresses(
            Duration::from_secs(args.timeout),
            args.allow_private_network,
            headers,
            &args.resolve,
        )
        .map_err(|source| RecoverError::Request {
            url: args.url.clone().unwrap_or_default(),
            source,
        })?;
        client.set_origin(args.url.as_deref().or(args.source_url.as_deref()));
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

fn load_headers(args: &Args) -> Result<HeaderMap> {
    let mut values = Vec::new();
    if let Some(path) = &args.headers_file {
        let text = fs::read_to_string(path).map_err(|source| RecoverError::ReadConfig {
            path: path.clone(),
            source,
        })?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|source| RecoverError::Config {
                path: path.clone(),
                reason: source.to_string(),
            })?;
        let object = value.as_object().ok_or_else(|| RecoverError::Config {
            path: path.clone(),
            reason: "header 文件必须是 JSON 对象".into(),
        })?;
        for (name, value) in object {
            let value = value.as_str().ok_or_else(|| RecoverError::Config {
                path: path.clone(),
                reason: format!("字段 {name} 的值必须是字符串"),
            })?;
            values.push((name.clone(), value.to_owned()));
        }
    }
    for (index, raw) in args.header.iter().enumerate() {
        let (name, value) = raw.split_once('=').ok_or_else(|| {
            RecoverError::Header(format!("第 {} 个 --header 格式应为 NAME=VALUE", index + 1))
        })?;
        values.push((name.trim().to_owned(), value.to_owned()));
    }
    network_policy::headers(&values).map_err(RecoverError::Header)
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
        .map(site_config::SiteSelectors::load)
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
    let document = crate::html::parse(&input.html);
    let requested_selector = settings
        .apply_overrides(&args.selector)?
        .unwrap_or_else(|| "auto".into());
    let selector =
        site::content_selector(&document, settings.content_selector().unwrap_or("auto"))?;
    logger.stage("parse", format!("使用正文选择器 {selector}"));
    let mut ir = parser::parse(
        &document,
        &selector,
        args.no_inference,
        Some(
            args.source_url
                .clone()
                .unwrap_or_else(|| input.source.clone()),
        ),
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

fn process_assets(
    ir: &mut Ir,
    args: &Args,
    output: &Path,
    network: Option<&Network>,
    logger: &Logger,
) -> Result<()> {
    let Some(network) = network else {
        return Ok(());
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
    Ok(())
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

fn report(parsed: &Parsed, rendered: &profile::Output, args: &Args, logger: &Logger) -> Result<()> {
    let ir = &parsed.ir;
    logger.warnings(&ir.warnings, &ir.original_nodes, &rendered.ir);
    logger.done(
        ir.blocks.iter().map(crate::model::Block::block_count).sum(),
        ir.attachments.len(),
        ir.images.len(),
    );
    if args.print_json_result {
        let mut result = crate::report::Report::completed(ir, rendered, args);
        result.source = serde_json::json!(parsed.source);
        result.selector = serde_json::json!(parsed.selector);
        result.requested_selector = serde_json::json!(parsed.requested_selector);
        result.print();
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
            || args.source_url.is_some()
            || !args.resolve.is_empty()
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
    let online =
        matches!(source, Source::Url(_)) || args.download_images || args.download_attachments;
    if !online && (!args.header.is_empty() || args.headers_file.is_some()) {
        return Err(RecoverError::Arguments(
            "--header 与 --headers-file 只用于网页或资源请求；本地 HTML 不发送请求头".into(),
        ));
    }
    if !online && !args.resolve.is_empty() {
        return Err(RecoverError::Arguments(
            "--resolve 只用于网页或资源请求；本地 HTML 不解析域名".into(),
        ));
    }
    let logger = Logger::with_filter(args.log_filter()?, args.color);
    let network = online.then(|| Network::new(args)).transpose()?;
    logger.stage("input", "开始读取输入");
    let mut parsed = parse(source.load(network.as_ref(), &logger)?, args, &logger)?;
    process_assets(&mut parsed.ir, args, output, network.as_ref(), &logger)?;
    let settings = args.output_options.resolve(args.profile);
    let rendered = if args.print_json_result {
        profile::prepare(&parsed.ir, settings)
    } else {
        profile::prepare_documents(&parsed.ir, settings)
    };
    let documents = export::render(&rendered.ir, settings, &args.exports);
    write_files(output, args.exports.files(documents, &parsed.ir)?)?;
    report(&parsed, &rendered, args, &logger)
}
