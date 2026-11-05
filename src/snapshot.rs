//! 记录解析后 DOM 的来源关系；文字值已经过实体解码。
use ego_tree::NodeId;
use scraper::{ElementRef, Node};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceNode {
    pub id: String,
    pub parent: Option<String>,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub attrs: HashMap<String, String>,
    #[serde(rename = "type")]
    pub kind: String,
}

pub fn validate(document: &Value) -> crate::error::Result<()> {
    use crate::error::SourceNodeError;
    let nodes: Vec<SourceNode> = match document.get("original_nodes") {
        Some(value) => serde_json::from_value(value.clone()).map_err(SourceNodeError::Decode)?,
        None => Vec::new(),
    };
    let mut index = HashMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if node.id.is_empty() {
            return Err(SourceNodeError::EmptyId {
                field: format!("original_nodes[{i}].id"),
            }
            .into());
        }
        if index.insert(node.id.as_str(), node).is_some() {
            return Err(SourceNodeError::DuplicateId {
                field: format!("original_nodes[{i}].id"),
                id: node.id.clone(),
            }
            .into());
        }
        if !matches!(node.kind.as_str(), "element" | "text" | "comment" | "other") {
            return Err(SourceNodeError::UnknownKind {
                field: format!("original_nodes[{i}].type"),
                kind: node.kind.clone(),
            }
            .into());
        }
    }
    for node in &nodes {
        for child in &node.children {
            if index
                .get(child.as_str())
                .is_none_or(|c| c.parent.as_deref() != Some(&node.id))
            {
                return Err(SourceNodeError::InvalidChild {
                    field: format!("original_nodes.{}.children", node.id),
                    child: child.clone(),
                }
                .into());
            }
        }
        if let Some(parent) = &node.parent {
            if index
                .get(parent.as_str())
                .is_none_or(|p| !p.children.contains(&node.id))
            {
                return Err(SourceNodeError::InvalidParent {
                    field: format!("original_nodes.{}.parent", node.id),
                    parent: parent.clone(),
                }
                .into());
            }
        }
    }
    let mut visited = std::collections::HashSet::new();
    let mut pending: Vec<_> = nodes.iter().filter(|n| n.parent.is_none()).collect();
    while let Some(node) = pending.pop() {
        if !visited.insert(&node.id) {
            return Err(SourceNodeError::InvalidTree.into());
        }
        pending.extend(node.children.iter().map(|id| index[id.as_str()]));
    }
    if visited.len() != nodes.len() {
        return Err(SourceNodeError::InvalidTree.into());
    }
    let mut pending = vec![
        ("blocks".to_string(), &document["blocks"]),
        ("images".into(), &document["images"]),
        ("attachments".into(), &document["attachments"]),
    ];
    while let Some((path, value)) = pending.pop() {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    let path = format!("{path}.{key}");
                    if key == "source_nodes" || key == "source_node" {
                        let refs: Vec<&Value> = if key == "source_node" {
                            vec![value]
                        } else {
                            value
                                .as_array()
                                .ok_or_else(|| SourceNodeError::ExpectedReferences {
                                    field: path.clone(),
                                })?
                                .iter()
                                .collect()
                        };
                        for id in refs {
                            let id = id.as_str().ok_or_else(|| SourceNodeError::ExpectedId {
                                field: path.clone(),
                            })?;
                            if !index.contains_key(id) {
                                return Err(SourceNodeError::MissingReference {
                                    field: path.clone(),
                                    id: id.into(),
                                }
                                .into());
                            }
                        }
                    } else {
                        pending.push((path, value));
                    }
                }
            }
            Value::Array(array) => pending.extend(
                array
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (format!("{path}[{i}]"), v)),
            ),
            _ => {}
        }
    }
    Ok(())
}

pub struct Snapshot {
    pub nodes: Vec<Value>,
    ids: HashMap<NodeId, String>,
}

impl Snapshot {
    /// 返回节点在原始节点列表中的 `n{index}` 标识；节点不在快照内时为 None。
    pub fn id_of(&self, id: NodeId) -> Option<&str> {
        self.ids.get(&id).map(String::as_str)
    }
    pub fn capture(root: ElementRef) -> Self {
        let ids: HashMap<_, _> = root
            .descendants()
            .enumerate()
            .map(|(index, node)| (node.id(), format!("n{index}")))
            .collect();
        let nodes = root
            .descendants()
            .map(|node| {
                let id = &ids[&node.id()];
                let parent = node.parent().and_then(|parent| ids.get(&parent.id()));
                let children: Vec<_> = node
                    .children()
                    .filter_map(|child| ids.get(&child.id()))
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
        &self.ids[&element.id()]
    }

    pub fn contains(&self, element: ElementRef) -> bool {
        self.ids.contains_key(&element.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    #[test]
    fn rejects_broken_references_duplicate_ids_and_cycles() {
        let node = json!({"id":"n0","type":"element","tag":"p","parent":null,"children":[]});
        let mut document = json!({"original_nodes":[node],"blocks":[{"source_nodes":["n0"]}]});
        validate(&document).unwrap();
        document["blocks"][0]["source_nodes"] = json!(["missing"]);
        assert_eq!(validate(&document).unwrap_err().code(), "E0310");
        document["blocks"] = json!([]);
        document["original_nodes"] = json!([node.clone(), node]);
        assert_eq!(validate(&document).unwrap_err().code(), "E0303");
        document["original_nodes"] = json!([
            {"id":"a","type":"element","parent":"b","children":["b"]},
            {"id":"b","type":"element","parent":"a","children":["a"]}
        ]);
        assert_eq!(validate(&document).unwrap_err().code(), "E0307");
    }

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

    #[test]
    fn inline_runs_use_snapshot_ids_for_text_breaks_and_images() {
        let html =
            "<div class='Article_Content'><p>前<br><img src='image.png' alt='图'>后</p></div>";
        let document = Html::parse_document(html);
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let block = &ir.blocks[0];
        assert!(block.runs.iter().any(|run| run.line_break));
        assert!(block.runs.iter().any(|run| run.image.is_some()));
        for run in &block.runs {
            for id in &run.source_nodes {
                let node = ir.original_nodes.iter().find(|node| node["id"] == *id);
                assert!(node.is_some(), "missing source node {id}");
            }
        }
        assert!(block
            .runs
            .iter()
            .flat_map(|run| run.source_nodes.iter())
            .all(|id| id.starts_with('n')));
    }
}
