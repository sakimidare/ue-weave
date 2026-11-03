//! Parsed DOM provenance. Text values are decoded; source bytes belong in the raw snapshot.
use scraper::{ElementRef, Node};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct Snapshot {
    pub nodes: Vec<Value>,
    ids: HashMap<String, String>,
}

impl Snapshot {
    pub fn capture(root: ElementRef) -> Self {
        let ids: HashMap<_, _> = root
            .descendants()
            .enumerate()
            .map(|(index, node)| (format!("{:?}", node.id()), format!("n{index}")))
            .collect();
        let nodes = root
            .descendants()
            .map(|node| {
                let id = &ids[&format!("{:?}", node.id())];
                let parent = node
                    .parent()
                    .and_then(|parent| ids.get(&format!("{:?}", parent.id())));
                let children: Vec<_> = node
                    .children()
                    .filter_map(|child| ids.get(&format!("{:?}", child.id())))
                    .collect();
                let mut record = json!({"id":id,"parent":parent,"children":children});
                match node.value() {
                    Node::Element(element) => {
                        record["type"] = json!("element");
                        record["tag"] = json!(element.name());
                        record["attrs"] = Value::Object(
                            element
                                .attrs()
                                .map(|(name, value)| (name.to_string(), json!(value)))
                                .collect(),
                        );
                    }
                    Node::Text(text) => {
                        record["type"] = json!("text");
                        record["text"] = json!(text.to_string());
                    }
                    Node::Comment(comment) => {
                        record["type"] = json!("comment");
                        record["text"] = json!(comment.to_string());
                    }
                    _ => {
                        record["type"] = json!("other");
                    }
                }
                record
            })
            .collect();
        Self { nodes, ids }
    }

    pub fn element_id(&self, element: ElementRef) -> &str {
        &self.ids[&format!("{:?}", element.id())]
    }

    pub fn contains(&self, element: ElementRef) -> bool {
        self.ids.contains_key(&format!("{:?}", element.id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    #[test]
    fn preserves_text_comments_order_and_resolvable_references() {
        let html = "<div class='Article_Content'>前<p data-x='1'>A&nbsp;<b>B</b><!--注释-->尾</p><p><a href='/a.pdf'>附件</a></p></div>";
        let document = Html::parse_document(html);
        let root = document
            .select(&Selector::parse(".Article_Content").unwrap())
            .next()
            .unwrap();
        let snapshot = Snapshot::capture(root);
        assert_eq!(snapshot.nodes[0]["id"], "n0");
        assert!(snapshot.nodes[0]["parent"].is_null());
        let text: String = snapshot
            .nodes
            .iter()
            .filter(|n| n["type"] == "text")
            .map(|n| n["text"].as_str().unwrap())
            .collect();
        assert_eq!(text, "前A\u{a0}B尾附件");
        assert!(snapshot
            .nodes
            .iter()
            .any(|n| n["type"] == "comment" && n["text"] == "注释"));
        for parent in &snapshot.nodes {
            for child_id in parent["children"].as_array().unwrap() {
                let child = snapshot
                    .nodes
                    .iter()
                    .find(|n| n["id"] == *child_id)
                    .unwrap();
                assert_eq!(child["parent"], parent["id"]);
            }
        }
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        for reference in ir.blocks.iter().flat_map(|b| &b.source_nodes) {
            assert!(ir.original_nodes.iter().any(|n| n["id"] == *reference));
        }
        let attachment_id = &ir.attachments[0]["source_nodes"][0];
        assert!(ir
            .original_nodes
            .iter()
            .any(|n| n["id"] == *attachment_id && n["tag"] == "a"));
        assert_eq!(
            ir.original_nodes,
            crate::parser::parse(&Html::parse_document(html), ".Article_Content", false, None)
                .unwrap()
                .original_nodes
        );
    }
}
