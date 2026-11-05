use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Error)]
#[serde(rename_all = "kebab-case")]
pub enum WarningKind {
    #[error("无法识别网页编码")]
    UnknownEncoding,
    #[error("部分文字解码失败")]
    DecodeReplacement,
    #[error("导出正文的文字或顺序发生变化")]
    ContentIntegrityMismatch,
    #[error("导出资源的地址或数量发生变化")]
    ResourceIntegrityMismatch,
    #[error("图片下载失败")]
    ImageDownload,
    #[error("附件下载失败")]
    AttachmentDownload,
    #[error("附件属性无法解析")]
    AttachmentMetadataUnparsed,
    #[error("网页基址无效")]
    InvalidBaseUrl,
    #[error("部分网页结构尚未支持")]
    UnsupportedStructure,
    #[error("目标格式尚未支持此排版")]
    ExportLimitation,
    #[error("样式声明格式错误")]
    MalformedStyle,
    #[error("已过滤事件属性或不可用地址")]
    UnsafeAttribute,
    #[error("已过滤不可用样式")]
    UnsafeStyle,
    #[error("此样式尚未支持")]
    UnmappedStyle,
    #[error("资源文件类型或文件名已调整")]
    AssetFormat,
    #[error("检测到零宽字符")]
    PossibleEditorFillCharacter,
    #[error("已过滤脚本或嵌入内容")]
    ExcludedElement,
    #[error("自定义列表标记需要检查")]
    CustomListMarker,
    #[error("嵌套过深，已提取文本")]
    NestingLimit,
}
impl WarningKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::UnknownEncoding => "W0001",
            Self::DecodeReplacement => "W0002",
            Self::ContentIntegrityMismatch => "W0003",
            Self::ResourceIntegrityMismatch => "W0004",
            Self::ImageDownload => "W0005",
            Self::AttachmentDownload => "W0006",
            Self::AttachmentMetadataUnparsed => "W0007",
            Self::InvalidBaseUrl => "W0008",
            Self::UnsupportedStructure => "W0009",
            Self::ExportLimitation => "W0010",
            Self::MalformedStyle => "W0011",
            Self::UnsafeAttribute => "W0012",
            Self::UnsafeStyle => "W0013",
            Self::UnmappedStyle => "W0014",
            Self::AssetFormat => "W0015",
            Self::PossibleEditorFillCharacter => "W0016",
            Self::ExcludedElement => "W0017",
            Self::CustomListMarker => "W0018",
            Self::NestingLimit => "W0019",
        }
    }
}
