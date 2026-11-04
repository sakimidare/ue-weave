//! 生成格式化的 DOM 片段及诊断标注范围。
use crate::snapshot::SourceNode as Node;
use serde_json::Value;
use std::{collections::HashMap, ops::Range};

pub(super) struct Context {
    nodes: HashMap<String, Node>,
}

pub(super) struct Location {
    pub text: String,
    pub range: Range<usize>,
    pub label: String,
}

impl Context {
    pub(super) fn ancestors(&self, id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut current = self.nodes.get(id);
        while let Some(node) = current {
            if result.contains(&node.id) {
                break;
            }
            result.push(node.id.clone());
            current = node.parent.as_ref().and_then(|id| self.nodes.get(id));
        }
        result
    }
    pub(super) fn new(records: &[Value]) -> Self {
        Self {
            nodes: records
                .iter()
                .filter_map(|n| serde_json::from_value::<Node>(n.clone()).ok())
                .map(|n| (n.id.clone(), n))
                .collect(),
        }
    }

    pub(super) fn locate(
        &self,
        id: &str,
        attribute: Option<&str>,
        original: Option<&str>,
    ) -> Option<Location> {
        let node = self.nodes.get(id)?;
        // 包含直接父标签，并保留目标附近的实际相邻标签。
        let root = node
            .parent
            .as_ref()
            .and_then(|id| self.nodes.get(id))
            .unwrap_or(node);
        let mut location = Location {
            text: String::new(),
            range: 0..0,
            label: format!("节点 {id}"),
        };
        let mut pending = vec![(root.id.as_str(), 0usize, false, root.id == id)];
        let mut expanded = 0usize;
        while let Some((current, depth, closing, in_target)) = pending.pop() {
            if current.is_empty() {
                location
                    .text
                    .push_str(&format!("{}…\n", "  ".repeat(depth.min(32))));
                continue;
            }
            let Some(current_node) = self.nodes.get(current) else {
                continue;
            };
            let indent = "  ".repeat(depth.min(32));
            if closing {
                location
                    .text
                    .push_str(&format!("{indent}</{}>\n", current_node.tag));
                continue;
            }
            location.text.push_str(&indent);
            let start = location.text.len();
            let mut selected = None;
            match current_node.kind.as_str() {
                "element" => {
                    location.text.push('<');
                    location.text.push_str(&current_node.tag);
                    let mut attrs: Vec<_> = current_node.attrs.iter().collect();
                    attrs.sort_by_key(|(name, _)| *name);
                    for (name, value) in attrs {
                        location.text.push(' ');
                        let attr_start = location.text.len();
                        location.text.push_str(name);
                        location.text.push_str("=\"");
                        let value_start = location.text.len();
                        location.text.push_str(&escape(value));
                        let value_end = location.text.len();
                        location.text.push('"');
                        if current == id
                            && (attribute == Some(name)
                                || (name == "style"
                                    && attribute
                                        .is_some_and(|a| !current_node.attrs.contains_key(a))))
                        {
                            selected = Some(
                                unique_range(&location.text, value_start..value_end, original)
                                    .unwrap_or(attr_start..location.text.len()),
                            );
                        } else if current == id && attribute.is_none() && selected.is_none() {
                            selected =
                                unique_range(&location.text, value_start..value_end, original);
                        }
                    }
                    location.text.push('>');
                }
                "text" => location.text.push_str(&escape(&current_node.text)),
                "comment" => location
                    .text
                    .push_str(&format!("<!--{}-->", escape(&current_node.text))),
                _ => {}
            }
            if current == id {
                let whole = start..location.text.len();
                location.range = selected
                    .or_else(|| unique_range(&location.text, whole.clone(), original))
                    .unwrap_or(whole);
            }
            if current_node.kind == "element"
                && current != root.id
                && !in_target
                && current != id
                && !is_void(&current_node.tag)
            {
                if !current_node.children.is_empty() {
                    location.text.push('…');
                }
                location
                    .text
                    .push_str(&format!("</{}>\n", current_node.tag));
                continue;
            }
            location.text.push('\n');
            if current_node.kind == "element" && !is_void(&current_node.tag) {
                let in_target = in_target || current == id;
                pending.push((current, depth, true, in_target));
                if current == root.id && current != id {
                    let children: Vec<_> = current_node
                        .children
                        .iter()
                        .filter(|child| {
                            child.as_str() == id
                                || self
                                    .nodes
                                    .get(*child)
                                    .is_some_and(|n| n.kind != "text" || !n.text.trim().is_empty())
                        })
                        .collect();
                    if let Some(index) = children.iter().position(|child| child.as_str() == id) {
                        let start = index.saturating_sub(1);
                        let end = (index + 2).min(children.len());
                        if end < children.len() {
                            pending.push(("", depth + 1, false, false));
                        }
                        pending.extend(
                            children[start..end].iter().rev().map(|child| {
                                (child.as_str(), depth + 1, false, child.as_str() == id)
                            }),
                        );
                        if start > 0 {
                            pending.push(("", depth + 1, false, false));
                        }
                    }
                } else if in_target {
                    expanded += 1;
                    let container_attribute = current == id
                        && attribute.is_some()
                        && matches!(
                            current_node.tag.as_str(),
                            "table"
                                | "thead"
                                | "tbody"
                                | "tfoot"
                                | "tr"
                                | "td"
                                | "th"
                                | "div"
                                | "section"
                                | "article"
                        );
                    if container_attribute
                        || expanded >= 8
                        || depth >= 6
                        || current_node.children.len() > 8
                    {
                        if !current_node.children.is_empty() {
                            location.text.push_str(&format!("{indent}  …\n"));
                        }
                    } else {
                        pending.extend(
                            current_node
                                .children
                                .iter()
                                .rev()
                                .map(|child| (child.as_str(), depth + 1, false, true)),
                        );
                    }
                } else if !current_node.children.is_empty() {
                    location.text.push_str(&format!("{indent}  …\n"));
                }
            }
        }
        Some(location)
    }
}

fn unique_range(text: &str, range: Range<usize>, original: Option<&str>) -> Option<Range<usize>> {
    let needle = escape(original.filter(|s| !s.is_empty())?);
    let mut matches = text[range.clone()].match_indices(&needle);
    let (offset, _) = matches.next()?;
    matches
        .next()
        .is_none()
        .then_some(range.start + offset..range.start + offset + needle.len())
}

fn escape(value: &str) -> String {
    let value: String = value
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '�'
            } else {
                c
            }
        })
        .collect();
    html_escape::encode_double_quoted_attribute(&value).replace('\u{a0}', "&nbsp;")
}

fn is_void(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_parent_keeps_neighbors_and_expands_target_text() {
        let html = format!("<table><tr><td>{}<p id='target' style='text-align:right'><span>东南大学教务处</span></p><p>日期</p><p>尾段</p></td></tr></table>", "<p>前段</p>\n".repeat(50));
        let document = scraper::Html::parse_fragment(&html);
        let snapshot = crate::snapshot::Snapshot::capture(document.root_element());
        let context = Context::new(&snapshot.nodes);
        let target = snapshot
            .nodes
            .iter()
            .find(|n| n["attrs"]["id"] == "target")
            .unwrap();
        let location = context
            .locate(target["id"].as_str().unwrap(), None, None)
            .unwrap();
        assert!(location.text.contains("东南大学教务处"));
        assert_eq!(location.text.matches("<p>").count(), 2);
        assert!(location.text.lines().count() < 15);
        assert_eq!(
            &location.text[location.range],
            "<p id=\"target\" style=\"text-align:right\">"
        );
    }
    #[test]
    fn html_context_marks_attributes_entities_and_empty_nodes() {
        let html = scraper::Html::parse_fragment(
            "<div><p style='color:red'>前文&nbsp;😀<b>强调</b></p><p></p><img alt='a&amp;b'></div>",
        );
        let snapshot = crate::snapshot::Snapshot::capture(html.root_element());
        let context = Context::new(&snapshot.nodes);
        for node in &snapshot.nodes {
            let id = node["id"].as_str().unwrap();
            let loc = context.locate(id, None, None).unwrap();
            assert!(loc.text.is_char_boundary(loc.range.start));
            assert!(loc.text.is_char_boundary(loc.range.end));
            if node["attrs"]["style"].is_string() {
                let loc = context.locate(id, Some("color"), Some("red")).unwrap();
                assert_eq!(&loc.text[loc.range], "red");
                assert!(loc.text.contains("style=\"color:red\""));
            }
            if node["type"] == "text" && node["text"].as_str().unwrap().contains('\u{a0}') {
                assert!(loc.text.contains("&nbsp;"));
                assert!(loc.text.contains("<p style="));
            }
            if node["tag"] == "img" {
                assert!(!loc.text.contains("</img>"));
            }
        }
    }

    #[test]
    fn repeated_text_is_selected_by_node_id() {
        let html = scraper::Html::parse_fragment("<p>重复<b>重复</b>重复</p>");
        let snapshot = crate::snapshot::Snapshot::capture(html.root_element());
        let context = Context::new(&snapshot.nodes);
        let text_nodes: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|n| n["type"] == "text")
            .collect();
        for node in &text_nodes {
            let loc = context
                .locate(node["id"].as_str().unwrap(), None, Some("重复"))
                .unwrap();
            assert_eq!(&loc.text[loc.range], "重复");
        }
        let first = context
            .locate(text_nodes[0]["id"].as_str().unwrap(), None, None)
            .unwrap();
        let last = context
            .locate(text_nodes[2]["id"].as_str().unwrap(), None, None)
            .unwrap();
        assert_ne!(first.range, last.range);
    }
}
