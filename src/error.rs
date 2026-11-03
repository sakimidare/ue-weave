use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecoverError {
    #[error("读取 IR JSON 失败：{path}: {source}")]
    ReadIr {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("输入参数错误：{0}")]
    Arguments(String),
    #[error("读取本地 HTML 文件失败：{path}: {source}")]
    ReadInput {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("请求 URL 失败：{url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("URL 返回 HTTP 状态 {status}：{url}")]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },
    #[error("读取 URL 响应正文失败：{url}: {source}")]
    ReadResponse {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("CSS 选择器无效：{selector}")]
    InvalidSelector { selector: String },
    #[error(
        "正文选择器未找到：{selector}。请确认页面是文章详情页，或使用 --selector 指定正文容器"
    )]
    ContentNotFound { selector: String },
    #[error("创建输出目录失败：{path}: {source}")]
    CreateOutput {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("写入输出文件失败：{path}: {source}")]
    WriteOutput {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("生成 JSON 失败：{file}: {source}")]
    Serialize {
        file: String,
        #[source]
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, RecoverError>;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("资源 URL 无效：{0}")]
    Url(#[from] url::ParseError),
    #[error("资源需要 HTTP/HTTPS URL")]
    Scheme,
    #[error("资源请求失败：{0}")]
    Request(#[from] reqwest::Error),
    #[error("资源超过 {limit} 字节限制")]
    TooLarge { limit: u64 },
    #[error("读取资源响应失败：{0}")]
    Read(#[source] io::Error),
    #[error("写入资源失败：{path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("无法分配资源文件名")]
    NameExhausted,
}

impl AssetError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Url(_) => "invalid_url",
            Self::Scheme => "unsupported_scheme",
            Self::Request(error) if error.is_status() => "http_status",
            Self::Request(error) if error.is_timeout() => "timeout",
            Self::Request(_) => "request_failed",
            Self::TooLarge { .. } => "size_limit",
            Self::Read(_) => "read_response",
            Self::Write { .. } => "write_file",
            Self::NameExhausted => "filename_exhausted",
        }
    }
}

impl RecoverError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Arguments(_) => "invalid_arguments",
            Self::ReadIr { .. } => "invalid_ir",
            Self::ReadInput { .. } => "read_input",
            Self::Request { .. } => "request_failed",
            Self::HttpStatus { .. } => "http_status",
            Self::ReadResponse { .. } => "read_response",
            Self::InvalidSelector { .. } => "invalid_selector",
            Self::ContentNotFound { .. } => "content_not_found",
            Self::CreateOutput { .. } => "create_output",
            Self::WriteOutput { .. } => "write_output",
            Self::Serialize { .. } => "serialize",
        }
    }
}
