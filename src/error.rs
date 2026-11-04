use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
/// 读取、解析、网络请求和导出阶段的错误。
pub enum RecoverError {
    #[error("没有找到编号 {code} 的解释。示例：ue-weave --explain N0002")]
    UnknownDiagnosticCode { code: String },
    #[error(transparent)]
    SourceNode(#[from] SourceNodeError),
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

/// 文章处理操作共用的返回类型。
pub type Result<T> = std::result::Result<T, RecoverError>;

#[derive(Debug, Error)]
/// 单个资源的下载、校验或保存错误。
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
            Self::ContentType(_) => "E0201",
            Self::Network(error) => error.kind.code(),
            Self::Url(_) => NetworkKind::InvalidUrl.code(),
            Self::Scheme => "E0202",
            Self::Write { .. } => "E0203",
            Self::NameExhausted => "E0204",
        }
    }
}

impl RecoverError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownDiagnosticCode { .. } => "E0011",
            Self::SourceNode(error) => error.code(),
            Self::Network(error) => error.kind.code(),
            Self::ReadConfig { .. } => "E0002",
            Self::Config { .. } => "E0003",
            Self::Arguments(_) => "E0001",
            Self::ReadIr { .. } => "E0012",
            Self::InvalidIr { .. } => "E0004",
            Self::ReadInput { .. } => "E0005",
            Self::Request { .. } => "E0013",
            Self::InvalidSelector { .. } => "E0006",
            Self::ContentNotFound { .. } => "E0007",
            Self::CreateOutput { .. } => "E0008",
            Self::WriteOutput { .. } => "E0009",
            Self::Serialize { .. } => "E0010",
        }
    }
}

#[derive(Debug, Error)]
/// 文章 JSON 中来源节点的结构或引用错误。
pub enum SourceNodeError {
    #[error("节点记录格式错误：original_nodes: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("节点 ID 为空：{field}")]
    EmptyId { field: String },
    #[error("节点 ID 重复：{field}: {id}")]
    DuplicateId { field: String, id: String },
    #[error("未知节点类型：{field}: {kind}")]
    UnknownKind { field: String, kind: String },
    #[error("子节点不存在或父节点不匹配：{field}: {child}")]
    InvalidChild { field: String, child: String },
    #[error("父节点不存在或未包含此子节点：{field}: {parent}")]
    InvalidParent { field: String, parent: String },
    #[error("节点关系存在重复或循环：original_nodes")]
    InvalidTree,
    #[error("需要节点 ID 数组：{field}")]
    ExpectedReferences { field: String },
    #[error("需要字符串节点 ID：{field}")]
    ExpectedId { field: String },
    #[error("引用的节点不存在：{field}: {id}")]
    MissingReference { field: String, id: String },
}

impl SourceNodeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Decode(_) => "E0301",
            Self::EmptyId { .. } => "E0302",
            Self::DuplicateId { .. } => "E0303",
            Self::UnknownKind { .. } => "E0304",
            Self::InvalidChild { .. } => "E0305",
            Self::InvalidParent { .. } => "E0306",
            Self::InvalidTree => "E0307",
            Self::ExpectedReferences { .. } => "E0308",
            Self::ExpectedId { .. } => "E0309",
            Self::MissingReference { .. } => "E0310",
        }
    }
}

#[derive(Debug, Error)]
#[error("网络请求失败 [{kind}]（尝试 {attempts} 次）：{url}: {detail}")]
/// 一次请求的错误类型、地址、重试次数和底层错误链。
pub struct NetworkError {
    pub kind: NetworkKind,
    pub url: String,
    pub attempts: u8,
    pub status: Option<u16>,
    pub detail: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl NetworkError {
    pub fn details(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.kind.code(),
            "url": self.url,
            "attempts": self.attempts,
            "http_status": self.status,
            "cause": self.detail,
            "kind": self.kind,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, thiserror::Error)]
/// 网络错误分类，决定日志编号和是否重试。
pub enum NetworkKind {
    #[serde(rename = "timeout")]
    #[error("请求超时")]
    Timeout,
    #[serde(rename = "dns")]
    #[error("域名解析失败")]
    Dns,
    #[serde(rename = "tls")]
    #[error("TLS 握手失败")]
    Tls,
    #[serde(rename = "connect")]
    #[error("连接失败")]
    Connect,
    #[serde(rename = "read_response")]
    #[error("读取响应失败")]
    ReadResponse,
    #[serde(rename = "http_status")]
    #[error("HTTP 响应状态异常")]
    HttpStatus,
    #[serde(rename = "size_limit")]
    #[error("响应超过大小限制")]
    SizeLimit,
    #[serde(rename = "invalid_url")]
    #[error("URL 格式无效")]
    InvalidUrl,
    #[serde(rename = "network_policy")]
    #[error("目标地址受网络选项限制")]
    NetworkPolicy,
    #[serde(rename = "request_failed")]
    #[error("请求失败")]
    RequestFailed,
}
impl NetworkKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Timeout => "E0101",
            Self::Dns => "E0102",
            Self::Tls => "E0103",
            Self::Connect => "E0104",
            Self::ReadResponse => "E0105",
            Self::HttpStatus => "E0106",
            Self::SizeLimit => "E0107",
            Self::InvalidUrl => "E0108",
            Self::NetworkPolicy => "E0109",
            Self::RequestFailed => "E0110",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{").unwrap_err()
    }

    #[test]
    fn distinct_error_categories_have_unique_explained_codes() {
        let errors = [
            RecoverError::Arguments(String::new()),
            RecoverError::ReadConfig {
                path: PathBuf::new(),
                source: io::Error::other("读取失败"),
            },
            RecoverError::Config {
                path: PathBuf::new(),
                reason: String::new(),
            },
            RecoverError::InvalidIr {
                field: String::new(),
                reason: String::new(),
            },
            RecoverError::ReadInput {
                path: PathBuf::new(),
                source: io::Error::other("读取失败"),
            },
            RecoverError::InvalidSelector {
                selector: String::new(),
            },
            RecoverError::ContentNotFound {
                selector: String::new(),
            },
            RecoverError::CreateOutput {
                path: PathBuf::new(),
                source: io::Error::other("创建失败"),
            },
            RecoverError::WriteOutput {
                path: PathBuf::new(),
                source: io::Error::other("写入失败"),
            },
            RecoverError::Serialize {
                file: String::new(),
                source: json_error(),
            },
            RecoverError::UnknownDiagnosticCode {
                code: String::new(),
            },
            RecoverError::ReadIr {
                path: PathBuf::new(),
                source: json_error(),
            },
            RecoverError::Request {
                url: String::new(),
                source: reqwest::blocking::Client::builder()
                    .user_agent("\n")
                    .build()
                    .unwrap_err(),
            },
        ];
        let nodes = [
            SourceNodeError::Decode(json_error()),
            SourceNodeError::EmptyId {
                field: String::new(),
            },
            SourceNodeError::DuplicateId {
                field: String::new(),
                id: String::new(),
            },
            SourceNodeError::UnknownKind {
                field: String::new(),
                kind: String::new(),
            },
            SourceNodeError::InvalidChild {
                field: String::new(),
                child: String::new(),
            },
            SourceNodeError::InvalidParent {
                field: String::new(),
                parent: String::new(),
            },
            SourceNodeError::InvalidTree,
            SourceNodeError::ExpectedReferences {
                field: String::new(),
            },
            SourceNodeError::ExpectedId {
                field: String::new(),
            },
            SourceNodeError::MissingReference {
                field: String::new(),
                id: String::new(),
            },
        ];
        let assets = [
            AssetError::ContentType(String::new()),
            AssetError::Scheme,
            AssetError::Write {
                path: PathBuf::new(),
                source: io::Error::other("写入失败"),
            },
            AssetError::NameExhausted,
        ];
        let network = [
            NetworkKind::Timeout,
            NetworkKind::Dns,
            NetworkKind::Tls,
            NetworkKind::Connect,
            NetworkKind::ReadResponse,
            NetworkKind::HttpStatus,
            NetworkKind::SizeLimit,
            NetworkKind::InvalidUrl,
            NetworkKind::NetworkPolicy,
            NetworkKind::RequestFailed,
        ];
        let codes: Vec<_> = errors
            .iter()
            .map(RecoverError::code)
            .chain(nodes.iter().map(SourceNodeError::code))
            .chain(assets.iter().map(AssetError::code))
            .chain(network.iter().map(|kind| kind.code()))
            .collect();
        let unique: std::collections::BTreeSet<_> = codes.iter().collect();
        assert_eq!(codes.len(), unique.len());
        for code in codes {
            assert!(crate::explain::lookup(code).is_ok(), "{code}");
        }
        for node in nodes {
            let code = node.code();
            assert_eq!(RecoverError::SourceNode(node).code(), code);
        }
        for kind in network {
            let make = || NetworkError {
                kind,
                url: String::new(),
                attempts: 1,
                status: None,
                detail: String::new(),
                source: None,
            };
            assert_eq!(RecoverError::Network(make()).code(), kind.code());
            assert_eq!(AssetError::Network(make()).code(), kind.code());
        }
        assert_eq!(
            AssetError::Url(url::Url::parse(":").unwrap_err()).code(),
            NetworkKind::InvalidUrl.code()
        );
    }
}
