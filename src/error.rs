use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecoverError {
    #[error("IR 校验失败：{field}: {reason}")]
    InvalidIr { field: String, reason: String },
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error("读取站点配置失败：{path}: {source}")]
    ReadConfig {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("站点配置无效：{path}: {reason}")]
    Config { path: PathBuf, reason: String },
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
    #[error("CSS 选择器无效：{selector}")]
    InvalidSelector { selector: String },
    #[error("正文选择器未找到：{selector}。可使用 --selector content=CSS 指定正文容器")]
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
    #[error("资源类型校验失败：{0}")]
    ContentType(String),
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error("资源 URL 无效：{0}")]
    Url(#[from] url::ParseError),
    #[error("资源需要 HTTP/HTTPS URL")]
    Scheme,
    #[error("写入资源失败：{path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("资源文件名冲突次数达到上限")]
    NameExhausted,
}

impl AssetError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ContentType(_) => "content_type",
            Self::Network(error) => error.kind,
            Self::Url(_) => "invalid_url",
            Self::Scheme => "unsupported_scheme",
            Self::Write { .. } => "write_file",
            Self::NameExhausted => "filename_exhausted",
        }
    }
}

impl RecoverError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Network(error) => error.kind,
            Self::ReadConfig { .. } => "read_config",
            Self::Config { .. } => "invalid_config",
            Self::Arguments(_) => "invalid_arguments",
            Self::ReadIr { .. } => "invalid_ir",
            Self::InvalidIr { .. } => "invalid_ir",
            Self::ReadInput { .. } => "read_input",
            Self::Request { .. } => "request_failed",
            Self::InvalidSelector { .. } => "invalid_selector",
            Self::ContentNotFound { .. } => "content_not_found",
            Self::CreateOutput { .. } => "create_output",
            Self::WriteOutput { .. } => "write_output",
            Self::Serialize { .. } => "serialize",
        }
    }
}

#[derive(Debug, Error)]
#[error("网络请求失败 [{kind}]（尝试 {attempts} 次）：{url}: {detail}")]
pub struct NetworkError {
    pub kind: &'static str,
    pub url: String,
    pub attempts: u8,
    pub status: Option<u16>,
    pub detail: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl NetworkError {
    pub fn details(&self) -> serde_json::Value {
        serde_json::json!({"url":self.url,"attempts":self.attempts,"http_status":self.status,"cause":self.detail,"kind":self.kind})
    }
}
