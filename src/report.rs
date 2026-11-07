//! 命令行运行报告，所有输入方式共用字段。
use crate::{cli::Args, diagnostic::DiagnosticKind, error::RecoverError, model::Ir, profile};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Default, Serialize)]
pub(crate) struct Report {
    pub status: &'static str,
    pub code: Option<String>,
    pub error: Option<String>,
    pub network: Value,
    pub source: Value,
    pub selector: Value,
    pub requested_selector: Value,
    pub output: Value,
    pub output_profile: Value,
    pub output_options: Value,
    pub counts: Value,
    pub warnings: Vec<Value>,
    pub notes: Vec<Value>,
    pub image_errors: Vec<Value>,
    pub attachment_errors: Vec<Value>,
    pub content_integrity: Value,
    pub resource_integrity: Value,
    pub output_changes: Vec<Value>,
}

impl Report {
    pub fn completed(ir: &Ir, rendered: &profile::Output, args: &Args) -> Self {
        let mut report = Self {
            status: "ok",
            source: json!(ir.source),
            output: json!(args.output),
            output_profile: json!(args.profile),
            output_options: json!(args.output_options.resolve(args.profile)),
            counts: json!({
                "blocks": ir.blocks.iter().map(crate::model::Block::block_count).sum::<usize>(),
                "images": ir.images.len(), "attachments": ir.attachments.len()
            }),
            content_integrity: ir
                .metadata
                .get("content_integrity")
                .cloned()
                .unwrap_or_default(),
            resource_integrity: ir
                .metadata
                .get("resource_integrity")
                .cloned()
                .unwrap_or_default(),
            output_changes: rendered.changes.clone(),
            ..Default::default()
        };
        for diagnostic in &ir.warnings {
            let value = serde_json::to_value(diagnostic).expect("diagnostic serialization");
            match diagnostic.kind {
                DiagnosticKind::Warning(kind) => {
                    use crate::diagnostic::WarningKind;
                    match kind {
                        WarningKind::ImageDownload => report.image_errors.push(value.clone()),
                        WarningKind::AttachmentDownload => {
                            report.attachment_errors.push(value.clone())
                        }
                        _ => {}
                    }
                    if matches!(
                        kind,
                        WarningKind::ImageDownload
                            | WarningKind::AttachmentDownload
                            | WarningKind::ContentIntegrityMismatch
                            | WarningKind::ResourceIntegrityMismatch
                    ) {
                        report.status = "partial";
                    }
                    report.warnings.push(value);
                }
                DiagnosticKind::Note(_) => report.notes.push(value),
            }
        }
        report
    }

    pub fn failed(error: &RecoverError) -> Self {
        Self {
            status: "error",
            code: Some(error.code().into()),
            error: Some(error.to_string()),
            network: match error {
                RecoverError::Network(error) => error.details(),
                _ => Value::Null,
            },
            ..Default::default()
        }
    }

    pub fn print(&self) {
        println!(
            "{}",
            serde_json::to_string_pretty(self).expect("report serialization")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Details, Diagnostic, WarningKind};
    use clap::Parser;

    #[test]
    fn integrity_failures_are_partial_but_style_warnings_are_not() {
        let html = scraper::Html::parse_document("<main><p>正文</p></main>");
        let source = crate::parser::parse(&html, "main", false, None).unwrap();
        let args =
            Args::try_parse_from(["ue-weave", "--input", "page.html", "--output", "out"]).unwrap();
        for (kind, status) in [
            (WarningKind::ContentIntegrityMismatch, "partial"),
            (WarningKind::ResourceIntegrityMismatch, "partial"),
            (WarningKind::UnmappedStyle, "ok"),
        ] {
            let mut ir = source.clone();
            ir.warnings.push(Diagnostic::new(kind, Details::default()));
            let rendered =
                profile::prepare_documents(&ir, args.output_options.resolve(args.profile));
            let report = Report::completed(&ir, &rendered, &args);
            assert_eq!(report.status, status);
        }
    }
}
