use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Error)]
#[serde(rename_all = "kebab-case")]
pub enum NoteKind {
    #[error("使用 --explain 查看编号解释")]
    ExplainHint,
    #[error("准备重试网络请求")]
    NetworkRetry,
    #[error("使用 --verbose 查看详情")]
    VerboseHint,
    #[error("检测到 Word 排版标记")]
    WordOrigin,
    #[error("已将末尾段落识别为署名")]
    SignatureInference,
    #[error("检测到空段落")]
    EmptyParagraph,
    #[error("已将 PDF 阅读器转为附件链接")]
    EmbeddedPdf,
    #[error("已转换 VML 图片尺寸")]
    VmlDimension,
    #[error("已转换 VML 图片")]
    VmlImage,
    #[error("已移除附件图标")]
    AttachmentIcon,
    #[error("已调整表格合并范围")]
    RowspanNormalized,
    #[error("检测到 Office 样式")]
    WordStyle,
    #[error("已将连续文字组合为段落")]
    InlineBlockGroup,
    #[error("已计算相对行距")]
    RelativeLineHeight,
    #[error("检测到 Word 段落格式")]
    WordParagraphStructure,
    #[error("网页未提供此文档属性")]
    Unrecoverable,
    #[error("已处理空标签")]
    EmptyElement,
    #[error("已清理异常字符串")]
    Undefinedundefined,
    #[error("已统计段落空白")]
    Whitespace,
    #[error("已处理编辑器扩展属性")]
    UeOrWordAttribute,
    #[error("已跳过样式注释")]
    CssCommentNormalization,
    #[error("已按优先级选择样式")]
    CssPriority,
    #[error("已读取图片尺寸")]
    ImageDimension,
    #[error("已读取表格样式")]
    TableStyle,
    #[error("已读取表格列宽")]
    ColumnWidth,
    #[error("已清理异常字体声明")]
    UndefinedundefinedStyle,
    #[error("已展开段落间距")]
    ParagraphMarginShorthand,
    #[error("已读取段落间距")]
    ParagraphSpacing,
    #[error("已读取单元格样式")]
    CellStyle,
    #[error("已转换 Word 样式")]
    WordStyleMapping,
}
impl NoteKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::ExplainHint => "N0030",
            Self::NetworkRetry => "N0028",
            Self::VerboseHint => "N0029",
            Self::WordOrigin => "N0001",
            Self::SignatureInference => "N0002",
            Self::EmptyParagraph => "N0003",
            Self::EmbeddedPdf => "N0004",
            Self::VmlDimension => "N0005",
            Self::VmlImage => "N0006",
            Self::AttachmentIcon => "N0007",
            Self::RowspanNormalized => "N0008",
            Self::WordStyle => "N0009",
            Self::InlineBlockGroup => "N0010",
            Self::RelativeLineHeight => "N0011",
            Self::WordParagraphStructure => "N0012",
            Self::Unrecoverable => "N0013",
            Self::EmptyElement => "N0014",
            Self::Undefinedundefined => "N0015",
            Self::Whitespace => "N0016",
            Self::UeOrWordAttribute => "N0017",
            Self::CssCommentNormalization => "N0018",
            Self::CssPriority => "N0019",
            Self::ImageDimension => "N0020",
            Self::TableStyle => "N0021",
            Self::ColumnWidth => "N0022",
            Self::UndefinedundefinedStyle => "N0023",
            Self::ParagraphMarginShorthand => "N0024",
            Self::ParagraphSpacing => "N0025",
            Self::CellStyle => "N0026",
            Self::WordStyleMapping => "N0027",
        }
    }
}
