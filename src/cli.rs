//! 命令行参数、退出状态与终端输出。
use crate::{error::RecoverError, export, log, pipeline, profile};
use clap::{CommandFactory, FromArgMatches, Parser};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ue-weave",
    version,
    about = "提取网页文章，导出 HTML、Markdown、Typst 或 JSON",
    after_help = "URL、--input、--input-ir 三选一。格式开关用 =false 关闭，细分开关优先于组合开关和预设。\n\n示例：\n  ue-weave \
        https://example.org/article.html -o output/\n  ue-weave --input page.html -o output/ --formats \
        markdown --profile aggressive"
)]
pub(crate) struct Args {
    /// 查看错误、警告或提示的解释，例如 N0002。
    #[arg(long, value_name = "CODE", help_heading = "运行信息")]
    pub(crate) explain: Option<String>,
    /// 文章网址（HTTP/HTTPS）。
    pub(crate) url: Option<String>,
    /// 读取本地 HTML 文件。
    #[arg(long, value_name = "FILE", help_heading = "输入与正文")]
    pub(crate) input: Option<PathBuf>,
    /// 本地 HTML 的原网页网址，用于补全链接和元数据。
    #[arg(long, requires = "input", value_name = "URL", value_parser = source_url, help_heading = "输入与正文")]
    pub(crate) source_url: Option<String>,
    /// 固定域名的连接地址，可重复，例如 jwc.seu.edu.cn=10.64.84.122。
    #[arg(long, value_name = "HOST=IP", value_parser = crate::network_policy::parse_address, help_heading = "资源与网络")]
    pub(crate) resolve: Vec<(String, std::net::IpAddr)>,
    /// 从 JSON 正文生成文档。
    #[arg(long, value_name = "FILE", help_heading = "输入与正文")]
    pub(crate) input_ir: Option<PathBuf>,
    /// JSON 的资源目录，默认使用 JSON 所在目录。
    #[arg(
        long,
        requires = "input_ir",
        value_name = "DIR",
        help_heading = "资源与网络"
    )]
    pub(crate) asset_root: Option<PathBuf>,
    /// 正文或字段选择器，可重复；如 '.article-body'、'title=h1'。
    #[arg(long, value_name = "[FIELD=]CSS", help_heading = "输入与正文")]
    pub(crate) selector: Vec<String>,
    /// 读取选择器 JSON 配置。
    #[arg(long, value_name = "FILE", help_heading = "输入与正文")]
    pub(crate) site_config: Option<PathBuf>,
    /// 重试次数，0–5；0 关闭。
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=5), value_name = "N", help_heading = "资源与网络")]
    pub(crate) retries: u8,
    /// 单次请求超时秒数，1–120。
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=120), value_name = "SECONDS", help_heading = "资源与网络")]
    pub(crate) timeout: u64,
    /// 允许请求校内网或本地地址。
    #[arg(long, help_heading = "资源与网络")]
    pub(crate) allow_private_network: bool,
    /// 为网页和资源请求添加 HTTP header，可重复，例如 `--header Referer=https://example.edu/`。
    #[arg(long, value_name = "NAME=VALUE", help_heading = "资源与网络")]
    pub(crate) header: Vec<String>,
    /// 从 JSON 对象文件读取 HTTP header，例如 `{ "Referer": "https://example.edu/" }`。
    #[arg(long, value_name = "FILE", help_heading = "资源与网络")]
    pub(crate) headers_file: Option<PathBuf>,
    /// 输出目录，自动创建；覆盖同名文档。
    #[arg(
        short,
        long,
        value_name = "DIR",
        required_unless_present = "explain",
        help_heading = "输出"
    )]
    pub(crate) output: Option<PathBuf>,
    /// 关闭标题、署名和缩进推断。
    #[arg(long, help_heading = "输入与正文")]
    pub(crate) no_inference: bool,
    /// 预设详细日志，可用 --log 覆盖。
    #[arg(long, help_heading = "运行信息")]
    pub(crate) verbose: bool,
    /// 预设只输出错误，可用 --log 开启其他日志。
    #[arg(long, conflicts_with = "verbose", help_heading = "运行信息")]
    pub(crate) quiet: bool,
    /// 日志开关，可重复或用逗号分隔：[topic[.severity]=]level，例如 style=list、style.warning=off。
    #[arg(
        long,
        value_name = "[TOPIC[.SEVERITY]=]LEVEL",
        help_heading = "运行信息"
    )]
    pub(crate) log: Vec<String>,
    /// 日志颜色：auto 自动、always 始终、never 关闭。
    #[arg(long, value_enum, default_value_t = log::Color::Auto, help_heading = "运行信息")]
    pub(crate) color: log::Color,
    /// 在终端输出 JSON 运行结果；正文用 --formats json。
    #[arg(long, help_heading = "运行信息")]
    pub(crate) json: bool,
    /// 下载图片到 images/，改用本地链接。
    #[arg(long, help_heading = "资源与网络")]
    pub(crate) download_images: bool,
    /// 下载附件到 attachments/，改用本地链接。
    #[arg(long, help_heading = "资源与网络")]
    pub(crate) download_attachments: bool,
    /// 选择 HTML、Markdown 和 Typst 的格式预设。
    #[arg(long, value_enum, default_value_t = profile::Profile::Conservative, help_heading = "输出")]
    pub(crate) profile: profile::Profile,
    #[command(flatten)]
    pub(crate) output_options: profile::Overrides,
    #[command(flatten)]
    pub(crate) exports: export::Options,
}

fn source_url(value: &str) -> Result<String, String> {
    let url = url::Url::parse(value).map_err(|e| e.to_string())?;
    crate::network_policy::check_url(&url, true).map_err(|e| e.to_string())?;
    Ok(url.into())
}

impl Args {
    pub(crate) fn log_filter(&self) -> crate::error::Result<log::Filter> {
        log::Filter::from_specs(&self.log, self.quiet, self.verbose)
            .map_err(crate::error::RecoverError::LogFilter)
    }
}

/// 运行命令行程序，并按执行结果设置进程退出状态。
pub fn main() {
    let argv: Vec<_> = std::env::args_os().collect();
    let args = match command()
        .try_get_matches_from(&argv)
        .and_then(|matches| Args::from_arg_matches(&matches))
    {
        Ok(args) => args,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                error.exit();
            }
            // Do not parse the arguments a second time with
            // `ignore_errors(true)`.  An invalid enum/value can leave
            // ArgMatches incomplete; querying it with `get_flag` or
            // `get_one` may panic inside clap.  These two options only affect
            // error presentation, so inspect their raw argv spelling instead.
            let json = raw_bool_flag(&argv, "json");
            let color = raw_color_option(&argv).unwrap_or_default();
            let filter = raw_log_filter(&argv);
            let message = error.to_string();
            let error =
                RecoverError::Arguments(message.trim_start_matches("error: ").trim().into());
            report_error(&error, json, &filter, color);
            std::process::exit(2);
        }
    };
    if let Err(error) = pipeline::run(&args) {
        let filter = args.log_filter().unwrap_or_default();
        report_error(&error, args.json, &filter, args.color);
        std::process::exit(1);
    }
}

/// 参数解析失败时尽力读取 `--log`、`--quiet`、`--verbose`，只影响错误提示。
fn raw_log_filter(argv: &[std::ffi::OsString]) -> log::Filter {
    let mut specs = Vec::new();
    let mut quiet = false;
    let mut verbose = false;
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        let Some(value) = arg.to_str() else {
            continue;
        };
        if value == "--" {
            break;
        }
        match value {
            "--quiet" => quiet = true,
            "--verbose" => verbose = true,
            "--log" => {
                if let Some(spec) = args.next().and_then(|value| value.to_str()) {
                    specs.push(spec.to_owned());
                }
            }
            _ => {
                if let Some(spec) = value.strip_prefix("--log=") {
                    specs.push(spec.to_owned());
                }
            }
        }
    }
    log::Filter::from_specs(&specs, quiet, verbose).unwrap_or_default()
}

fn raw_bool_flag(argv: &[std::ffi::OsString], name: &str) -> bool {
    let long = format!("--{name}");
    for arg in argv.iter().skip(1) {
        let Some(value) = arg.to_str() else {
            continue;
        };
        if value == "--" {
            break;
        }
        if value == long {
            return true;
        }
    }
    false
}

fn raw_color_option(argv: &[std::ffi::OsString]) -> Option<log::Color> {
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg.to_str() == Some("--") {
            break;
        }
        if let Some(value) = arg.to_str().and_then(|arg| arg.strip_prefix("--color=")) {
            return value.parse().ok();
        }
        if arg == "--color" {
            return args.next().and_then(|value| value.to_str()?.parse().ok());
        }
    }
    None
}

fn command() -> clap::Command {
    let command = Args::command();
    let conflicts: Vec<_> = command
        .get_arguments()
        .filter(|arg| {
            !matches!(
                arg.get_id().as_str(),
                "explain" | "color" | "help" | "version"
            )
        })
        .map(|arg| arg.get_id().clone())
        .collect();
    command.mut_arg("explain", |arg| arg.conflicts_with_all(conflicts))
}

fn report_error(error: &RecoverError, json: bool, filter: &log::Filter, color: log::Color) {
    if json {
        crate::report::Report::failed(error).print();
    }
    log::Logger::with_filter(filter.clone(), color).error(error);
}
