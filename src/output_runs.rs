//! 合并导出时格式相同的相邻文字，保留解析结果及来源记录。
use crate::model::Run;
use std::borrow::Cow;
use std::collections::HashSet;

fn mergeable(left: &Run, right: &Run) -> bool {
    !left.line_break
        && !right.line_break
        && left.image.is_none()
        && right.image.is_none()
        && left.link.is_none()
        && right.link.is_none()
        && left.inline_kind.is_none()
        && right.inline_kind.is_none()
        && left.image_attributes.is_none()
        && right.image_attributes.is_none()
        && left.bold == right.bold
        && left.italic == right.italic
        && left.underline == right.underline
        && left.strike == right.strike
        && left.font_family == right.font_family
        && left.font_size == right.font_size
        && left.color == right.color
        && left.background_color == right.background_color
        && left.vertical_align == right.vertical_align
        && left.source == right.source
}

pub(crate) fn coalesce(runs: &[Run]) -> Vec<Cow<'_, Run>> {
    let mut result: Vec<Cow<'_, Run>> = Vec::with_capacity(runs.len());
    let mut sources: Option<HashSet<String>> = None;
    for run in runs {
        if let Some(previous) = result.last_mut().filter(|last| mergeable(last, run)) {
            let previous = previous.to_mut();
            let sources =
                sources.get_or_insert_with(|| previous.source_nodes.iter().cloned().collect());
            previous.text.push_str(&run.text);
            for node in &run.source_nodes {
                if sources.insert(node.clone()) {
                    previous.source_nodes.push(node.clone());
                }
            }
        } else {
            sources = None;
            result.push(Cow::Borrowed(run));
        }
    }
    result
}

pub(crate) fn render(runs: &[Run], render_run: fn(&Run) -> String) -> String {
    coalesce(runs).iter().map(|run| render_run(run)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_sources_without_changing_input() {
        let runs = [
            Run {
                text: "202".into(),
                bold: true,
                source_nodes: vec!["n1".into()],
                ..Default::default()
            },
            Run {
                text: "3级".into(),
                bold: true,
                source_nodes: vec!["n1".into(), "n2".into()],
                ..Default::default()
            },
        ];
        let merged = coalesce(&runs);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "2023级");
        assert_eq!(merged[0].source_nodes, ["n1", "n2"]);
        assert_eq!(runs[0].text, "202");
        assert_eq!(runs[0].source_nodes, ["n1"]);
    }

    #[test]
    fn keeps_format_and_resource_boundaries() {
        let changes: &[fn(&mut Run)] = &[
            |r| r.bold = true,
            |r| r.italic = true,
            |r| r.underline = true,
            |r| r.strike = true,
            |r| r.font_family = Some("Arial".into()),
            |r| r.font_size = Some("12px".into()),
            |r| r.color = Some("red".into()),
            |r| r.background_color = Some("blue".into()),
            |r| r.vertical_align = Some("super".into()),
            |r| r.source = "inferred".into(),
        ];
        for change in changes {
            let left = Run {
                text: "甲".into(),
                ..Default::default()
            };
            let mut right = left.clone();
            change(&mut right);
            assert_eq!(coalesce(&[left, right]).len(), 2);
        }
        for change in [
            (|r: &mut Run| r.link = Some("https://example.org/".into())) as fn(&mut Run),
            |r| r.image = Some("image.png".into()),
            |r| r.line_break = true,
            |r| r.inline_kind = Some(crate::model::InlineKind::Code),
        ] {
            let mut run = Run {
                text: "甲".into(),
                ..Default::default()
            };
            change(&mut run);
            assert_eq!(coalesce(&[run.clone(), run]).len(), 2);
        }
    }
}
