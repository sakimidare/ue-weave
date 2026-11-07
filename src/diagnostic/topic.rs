//! 诊断编号到日志领域的映射，供 `--log` 过滤使用。
//!
//! 领域只描述“问题出在网页的哪一部分”，与警告/提示级别无关；
//! 级别由 `--log topic.severity` 或单独的 `--log warning` 选择。
//! 错误不受日志开关影响，始终输出。

use super::{DiagnosticKind, NoteKind, WarningKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Progress,
    Network,
    Encoding,
    Image,
    Attachment,
    Style,
    Structure,
    Integrity,
    Inference,
    Word,
}

/// 可作为 `--log` 名称的完整列表，用于错误提示。
pub const NAMES: &[&str] = &[
    "all",
    "progress",
    "warning",
    "note",
    "network",
    "encoding",
    "image",
    "attachment",
    "style",
    "structure",
    "integrity",
    "inference",
    "word",
];

impl Topic {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "progress" => Self::Progress,
            "network" => Self::Network,
            "encoding" => Self::Encoding,
            "image" => Self::Image,
            "attachment" => Self::Attachment,
            "style" => Self::Style,
            "structure" => Self::Structure,
            "integrity" => Self::Integrity,
            "inference" => Self::Inference,
            "word" => Self::Word,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Progress => "progress",
            Self::Network => "network",
            Self::Encoding => "encoding",
            Self::Image => "image",
            Self::Attachment => "attachment",
            Self::Style => "style",
            Self::Structure => "structure",
            Self::Integrity => "integrity",
            Self::Inference => "inference",
            Self::Word => "word",
        }
    }
}

pub fn topics(kind: DiagnosticKind) -> &'static [Topic] {
    match kind {
        DiagnosticKind::Warning(kind) => match kind {
            WarningKind::UnknownEncoding | WarningKind::DecodeReplacement => &[Topic::Encoding],
            WarningKind::ContentIntegrityMismatch | WarningKind::ResourceIntegrityMismatch => {
                &[Topic::Integrity]
            }
            WarningKind::ImageDownload => &[Topic::Image],
            WarningKind::AttachmentDownload | WarningKind::AttachmentMetadataUnparsed => {
                &[Topic::Attachment]
            }
            WarningKind::AssetFormat => &[Topic::Image, Topic::Attachment],
            WarningKind::InvalidBaseUrl
            | WarningKind::UnsupportedStructure
            | WarningKind::ExportLimitation
            | WarningKind::UnsafeAttribute
            | WarningKind::PossibleEditorFillCharacter
            | WarningKind::ExcludedElement
            | WarningKind::NestingLimit => &[Topic::Structure],
            WarningKind::MalformedStyle
            | WarningKind::UnsafeStyle
            | WarningKind::UnmappedStyle
            | WarningKind::CustomListMarker => &[Topic::Style],
        },
        DiagnosticKind::Note(kind) => match kind {
            NoteKind::NetworkRetry => &[Topic::Network],
            NoteKind::ExplainHint | NoteKind::VerboseHint => &[Topic::Progress],
            NoteKind::WordOrigin
            | NoteKind::WordStyle
            | NoteKind::WordParagraphStructure
            | NoteKind::WordStyleMapping
            | NoteKind::UeOrWordAttribute => &[Topic::Word],
            NoteKind::SignatureInference => &[Topic::Inference],
            NoteKind::EmbeddedPdf | NoteKind::AttachmentIcon | NoteKind::AttachmentLabel => {
                &[Topic::Attachment]
            }
            NoteKind::VmlDimension | NoteKind::VmlImage | NoteKind::ImageDimension => {
                &[Topic::Image]
            }
            NoteKind::EmptyParagraph
            | NoteKind::RowspanNormalized
            | NoteKind::InlineBlockGroup
            | NoteKind::Unrecoverable
            | NoteKind::EmptyElement
            | NoteKind::Undefinedundefined
            | NoteKind::Whitespace
            | NoteKind::EditorBookmark
            | NoteKind::CodeWhitespace
            | NoteKind::StaticField => &[Topic::Structure],
            NoteKind::PaginationStyle
            | NoteKind::ListStyle
            | NoteKind::RelativeLineHeight
            | NoteKind::CssCommentNormalization
            | NoteKind::CssPriority
            | NoteKind::TableStyle
            | NoteKind::ColumnWidth
            | NoteKind::UndefinedundefinedStyle
            | NoteKind::ParagraphMarginShorthand
            | NoteKind::ParagraphSpacing
            | NoteKind::CellStyle
            | NoteKind::IgnoredStyle => &[Topic::Style],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_unknown_names_fail() {
        for name in [
            "progress",
            "network",
            "encoding",
            "image",
            "attachment",
            "style",
            "structure",
            "integrity",
            "inference",
            "word",
        ] {
            let topic = Topic::parse(name).unwrap();
            assert_eq!(topic.name(), name);
        }
        assert!(Topic::parse("all").is_none());
        assert!(Topic::parse("asset").is_none());
        assert!(Topic::parse("media").is_none());
        assert!(Topic::parse("error").is_none());
    }

    #[test]
    fn a_domain_keeps_both_warnings_and_notes() {
        assert_eq!(
            topics(NoteKind::AttachmentIcon.into()),
            &[Topic::Attachment]
        );
        assert_eq!(
            topics(WarningKind::AttachmentDownload.into()),
            &[Topic::Attachment]
        );
        assert_eq!(topics(NoteKind::IgnoredStyle.into()), &[Topic::Style]);
        assert_eq!(topics(WarningKind::UnmappedStyle.into()), &[Topic::Style]);
        let shared = topics(WarningKind::AssetFormat.into());
        assert!(shared.contains(&Topic::Image) && shared.contains(&Topic::Attachment));
    }
}
