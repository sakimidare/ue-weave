//! 终端与 JSON 输出共用的诊断类型和编号。
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

mod details;
mod note;
mod topic;
mod warning;
pub use details::Details;
pub use note::NoteKind;
pub use topic::{topics, Topic, NAMES as TOPIC_NAMES};
pub use warning::WarningKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Error)]
#[serde(untagged)]
pub enum DiagnosticKind {
    #[error(transparent)]
    Warning(#[from] WarningKind),
    #[error(transparent)]
    Note(#[from] NoteKind),
}
impl DiagnosticKind {
    pub fn level(self) -> &'static str {
        match self {
            Self::Warning(_) => "warning",
            Self::Note(_) => "note",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Warning(kind) => kind.code(),
            Self::Note(kind) => kind.code(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    details: Value,
}
impl Diagnostic {
    pub fn new(kind: impl Into<DiagnosticKind>, details: Details) -> Self {
        let kind = kind.into();
        let mut details = serde_json::to_value(details).expect("诊断详情由可序列化字段组成");
        details["kind"] = serde_json::to_value(kind).expect("enum serialization");
        details["code"] = kind.code().into();
        details["level"] = kind.level().into();
        Self { kind, details }
    }
}
impl std::ops::Deref for Diagnostic {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.details
    }
}
impl Serialize for Diagnostic {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.details.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Diagnostic {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut details = Value::deserialize(deserializer)?;
        let kind: DiagnosticKind =
            serde_json::from_value(details["kind"].clone()).map_err(serde::de::Error::custom)?;
        let object = details
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("诊断详情需要对象"))?;
        for key in ["kind", "code", "level"] {
            object.remove(key);
        }
        let details = serde_json::from_value(details).map_err(serde::de::Error::custom)?;
        Ok(Self::new(kind, details))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_details_reject_unknown_fields_and_wrong_types() {
        for value in [
            serde_json::json!({"kind":"empty-element","reson":"拼写错误"}),
            serde_json::json!({"kind":"empty-element","removed":"true"}),
            serde_json::json!({"kind":"empty-element","source_nodes":[1]}),
        ] {
            assert!(serde_json::from_value::<Diagnostic>(value).is_err());
        }
        let diagnostic: Diagnostic = serde_json::from_value(serde_json::json!({
            "kind": "empty-element", "code":"wrong", "level":"warning", "removed":true
        }))
        .unwrap();
        assert_eq!(diagnostic["code"], "N0014");
        assert_eq!(diagnostic["level"], "note");
        assert_eq!(diagnostic["removed"], true);
    }
    #[test]
    fn diagnostic_codes_are_unique_and_unknown_kinds_fail() {
        let kinds: [DiagnosticKind; 56] = [
            NoteKind::IgnoredStyle.into(),
            NoteKind::AttachmentLabel.into(),
            WarningKind::UnknownEncoding.into(),
            WarningKind::DecodeReplacement.into(),
            WarningKind::ContentIntegrityMismatch.into(),
            WarningKind::ResourceIntegrityMismatch.into(),
            WarningKind::ImageDownload.into(),
            WarningKind::AttachmentDownload.into(),
            WarningKind::AttachmentMetadataUnparsed.into(),
            WarningKind::InvalidBaseUrl.into(),
            WarningKind::UnsupportedStructure.into(),
            WarningKind::ExportLimitation.into(),
            WarningKind::MalformedStyle.into(),
            WarningKind::UnsafeAttribute.into(),
            WarningKind::UnsafeStyle.into(),
            WarningKind::UnmappedStyle.into(),
            WarningKind::AssetFormat.into(),
            NoteKind::WordOrigin.into(),
            NoteKind::SignatureInference.into(),
            NoteKind::EmptyParagraph.into(),
            NoteKind::EmbeddedPdf.into(),
            NoteKind::VmlDimension.into(),
            NoteKind::VmlImage.into(),
            NoteKind::AttachmentIcon.into(),
            NoteKind::RowspanNormalized.into(),
            NoteKind::WordStyle.into(),
            NoteKind::InlineBlockGroup.into(),
            NoteKind::RelativeLineHeight.into(),
            NoteKind::WordParagraphStructure.into(),
            NoteKind::Unrecoverable.into(),
            NoteKind::EmptyElement.into(),
            NoteKind::Undefinedundefined.into(),
            WarningKind::PossibleEditorFillCharacter.into(),
            NoteKind::Whitespace.into(),
            WarningKind::ExcludedElement.into(),
            WarningKind::CustomListMarker.into(),
            WarningKind::NestingLimit.into(),
            NoteKind::UeOrWordAttribute.into(),
            NoteKind::CssCommentNormalization.into(),
            NoteKind::CssPriority.into(),
            NoteKind::ImageDimension.into(),
            NoteKind::TableStyle.into(),
            NoteKind::ColumnWidth.into(),
            NoteKind::UndefinedundefinedStyle.into(),
            NoteKind::ParagraphMarginShorthand.into(),
            NoteKind::ParagraphSpacing.into(),
            NoteKind::CellStyle.into(),
            NoteKind::WordStyleMapping.into(),
            NoteKind::NetworkRetry.into(),
            NoteKind::VerboseHint.into(),
            NoteKind::ExplainHint.into(),
            NoteKind::EditorBookmark.into(),
            NoteKind::CodeWhitespace.into(),
            NoteKind::PaginationStyle.into(),
            NoteKind::StaticField.into(),
            NoteKind::ListStyle.into(),
        ];
        let codes: std::collections::BTreeSet<_> = kinds.iter().map(|kind| kind.code()).collect();
        assert_eq!(codes.len(), kinds.len());
        for (level, prefix) in [("warning", 'W'), ("note", 'N')] {
            let mut actual: Vec<_> = kinds
                .iter()
                .filter(|kind| kind.level() == level)
                .map(|kind| kind.code())
                .collect();
            actual.sort_unstable();
            let expected: Vec<_> = (1..=actual.len())
                .map(|number| format!("{prefix}{number:04}"))
                .collect();
            assert_eq!(actual, expected);
        }
        for kind in kinds {
            assert_eq!(
                crate::explain::lookup(kind.code())
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap(),
                format!("{} {}", kind.code(), kind),
                "解释标题与日志标题不一致"
            );
            let warning = Diagnostic::new(
                kind,
                crate::diagnostic::Details {
                    reason: Some(("example").to_owned()),
                    ..Default::default()
                },
            );
            let value = serde_json::to_value(&warning).unwrap();
            assert_eq!(value["code"], kind.code());
            let restored: Diagnostic = serde_json::from_value(value).unwrap();
            assert_eq!(restored.kind, kind);
        }
        assert!(
            serde_json::from_value::<Diagnostic>(serde_json::json!({"kind":"misspelled"})).is_err()
        );
    }

    #[test]
    fn note_and_warning_enums_accept_only_their_own_kinds() {
        assert!(
            serde_json::from_value::<WarningKind>(serde_json::json!("signature-inference"))
                .is_err()
        );
        assert!(serde_json::from_value::<NoteKind>(serde_json::json!("image-download")).is_err());
        let note = Diagnostic::new(
            NoteKind::SignatureInference,
            crate::diagnostic::Details::default(),
        );
        let warning = Diagnostic::new(
            WarningKind::ImageDownload,
            crate::diagnostic::Details::default(),
        );
        assert!(matches!(
            note.kind,
            DiagnosticKind::Note(NoteKind::SignatureInference)
        ));
        assert!(matches!(
            warning.kind,
            DiagnosticKind::Warning(WarningKind::ImageDownload)
        ));
        assert_eq!(note["level"], "note");
        assert_eq!(warning["level"], "warning");
    }
}
