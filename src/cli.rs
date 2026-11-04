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
    /// 允许请求私网、回环和保留网段。
    #[arg(long, help_heading = "资源与网络")]
    pub(crate) allow_private_network: bool,
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
    /// 显示详细日志。
    #[arg(long, help_heading = "运行信息")]
    pub(crate) verbose: bool,
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
            let partial = command().ignore_errors(true).get_matches_from(&argv);
            let json = partial.get_flag("json");
            let color = partial
                .get_one::<log::Color>("color")
                .copied()
                .unwrap_or_default();
            let message = error.to_string();
            let error =
                RecoverError::Arguments(message.trim_start_matches("error: ").trim().into());
            report_error(&error, json, false, color);
            std::process::exit(2);
        }
    };
    if let Err(error) = pipeline::run(&args) {
        report_error(&error, args.json, args.verbose, args.color);
        std::process::exit(1);
    }
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

fn report_error(error: &RecoverError, json: bool, verbose: bool, color: log::Color) {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "status": "error", "code": error.code(), "error": error.to_string(),
                "network": match error { RecoverError::Network(error) => error.details(), _ => serde_json::Value::Null }
            })
        );
    }
    log::Logger::with_color(verbose, color).error(error);
}
