use super::context::Context;
use crate::diagnostic::Diagnostic;
use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};

pub(super) fn diagnostic(
    warning: &Diagnostic,
    context: &Context,
    output: &super::output_context::OutputContext,
    width: usize,
) -> String {
    let level = if warning.kind.level() == "warning" {
        Level::WARNING
    } else {
        Level::NOTE
    };
    let title = warning.kind.to_string();
    let pane_width = if width >= 100 { (width - 7) / 2 } else { width };
    let mut rendered = format!(
        "{}\n",
        Renderer::styled().render(&[Group::with_title(
            level.clone().primary_title(&title).id(warning.kind.code())
        )])
    );
    let mut footers: Vec<_> = super::fields(warning)
        .into_iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect();
    for id in warning["source_nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| id.as_str())
    {
        if let Some(location) = context.locate(
            id,
            warning["attribute"].as_str(),
            warning["original"].as_str(),
        ) {
            let cleaned = output.locate(id);
            if cleaned.is_empty() {
                let left = pane(&location.text, location.range.clone(), level.clone(), width);
                rendered.push_str(&left.text);
                rendered.push('\n');
            }
            if cleaned.len() > 1 {
                footers.push(format!(
                    "此节点关联 {} 处输出，右侧预览第一处",
                    cleaned.len()
                ));
            }
            for (text, range) in cleaned.into_iter().take(1) {
                let left = pane(
                    &location.text,
                    location.range.clone(),
                    level.clone(),
                    pane_width,
                );
                let right = pane(text, range, level.clone(), pane_width);
                rendered.push_str(&super::layout::pair(left, right, width));
            }
            footers.push(location.label);
        }
    }
    if let Some(url) = warning["url"].as_str() {
        footers.push(format!(
            "地址: {}",
            url.chars().filter(|c| !c.is_control()).collect::<String>()
        ));
    }
    let mut message = Group::with_level(Level::NOTE);
    for footer in &footers {
        message = message.element(Level::NOTE.message(footer));
    }
    if !footers.is_empty() {
        rendered.push_str(&format!("{}\n", Renderer::styled().render(&[message])));
    }
    rendered.push('\n');
    rendered
}

fn pane(
    text: &str,
    range: std::ops::Range<usize>,
    level: Level<'_>,
    width: usize,
) -> super::layout::Pane {
    let report = [Group::with_level(level).element(
        Snippet::source(text)
            .line_numbering(false)
            .fold(false)
            .annotation(AnnotationKind::Primary.span(range).label("对应位置")),
    )];
    let plain = Renderer::plain()
        .term_width(width)
        .render(&report)
        .to_string();
    let anchor = plain
        .lines()
        .position(|line| line.contains("对应位置"))
        .unwrap_or(0);
    let text = Renderer::styled()
        .term_width(width)
        .render(&report)
        .to_string();
    super::layout::Pane { text, anchor }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::WarningKind;
    #[test]
    fn container_attribute_has_one_compact_html_preview() {
        let source = format!("<div class='Article_Content'><table><tr><td style='padding:0px'>{}</td></tr></table></div>", "<p>重复正文</p>".repeat(40));
        let html = scraper::Html::parse_document(&source);
        let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
        let context = Context::new(&ir.original_nodes);
        let output = super::super::output_context::OutputContext::new(&ir, &context);
        let id = &ir.original_nodes.iter().find(|n| n["tag"] == "td").unwrap()["id"];
        let warning = Diagnostic::new(
            crate::diagnostic::NoteKind::CellStyle,
            crate::diagnostic::Details {
                source_nodes: vec![id.as_str().unwrap().to_owned()],
                attribute: Some(("padding").to_owned()),
                original: serde_json::json!("0px"),
                ..Default::default()
            },
        );
        let text = diagnostic(&warning, &context, &output, 120);
        assert_eq!(text.matches("<td").count(), 1);
        assert!(text.lines().count() < 25, "{text}");
        assert!(text.contains("40 处输出"));
    }
    #[test]
    fn renders_unicode_empty_nodes_and_attribute_context() {
        let html = scraper::Html::parse_fragment("<p style='color:red'>前文😀后文</p><p></p>");
        let snapshot = crate::snapshot::Snapshot::capture(html.root_element());
        let context = Context::new(&snapshot.nodes);
        let output = super::super::output_context::OutputContext::new(
            &crate::model::Ir::default(),
            &context,
        );
        for node in snapshot.nodes.iter().filter(|n| n["tag"] == "p") {
            let warning = Diagnostic::new(
                WarningKind::UnmappedStyle,
                crate::diagnostic::Details {
                    source_nodes: vec![node["id"].as_str().unwrap_or_default().to_owned()],
                    attribute: Some(("style").to_owned()),
                    original: serde_json::json!("color:red"),
                    ..Default::default()
                },
            );
            let text = diagnostic(&warning, &context, &output, 120);
            assert!(text.contains("W0014"));
            assert!(!text.contains("LL |"));
            assert!(!text.contains("-->"));
            assert!(!text.contains('│'));
            assert!(!text.contains("无对应输出文字"));
            assert!(text.contains("<p"));
            if node["attrs"]["style"].is_string() {
                assert!(text.contains("style=\"color:red\""));
            }
        }
    }
}
