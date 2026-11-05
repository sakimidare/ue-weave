//! 在递归生成正文结构前检查 HTML 深度。
use crate::{
    diagnostic::{Details, Diagnostic, WarningKind},
    model::{Block, Ir, Run},
    snapshot::Snapshot,
};
use ego_tree::iter::Edge;
use scraper::{ElementRef, Node};

pub const HTML_LIMIT: usize = 256;
pub const BLOCK_LIMIT: usize = 24;

pub fn exceeds(root: ElementRef) -> bool {
    let mut depth = 0usize;
    for edge in root.traverse() {
        match edge {
            Edge::Open(_) => {
                depth += 1;
                if depth > HTML_LIMIT {
                    return true;
                }
            }
            Edge::Close(_) => depth -= 1,
        }
    }
    false
}

pub fn fallback(root: ElementRef, snapshot: &Snapshot, ir: &mut Ir) {
    let mut runs = Vec::new();
    let mut skipped = 0usize;
    for edge in root.traverse() {
        match edge {
            Edge::Open(node) => {
                if skipped > 0 {
                    skipped += 1;
                    continue;
                }
                if ElementRef::wrap(node).is_some_and(|el| {
                    matches!(el.value().name(), "script" | "style" | "object" | "embed")
                }) {
                    skipped = 1;
                    continue;
                }
                let text = match node.value() {
                    Node::Text(text) => text.to_string(),
                    Node::Element(el) if el.name() == "br" => "\n".into(),
                    _ => continue,
                };
                runs.push(Run {
                    text,
                    source_nodes: snapshot
                        .id_of(node.id())
                        .map(str::to_owned)
                        .into_iter()
                        .collect(),
                    ..Default::default()
                });
            }
            Edge::Close(_) if skipped > 0 => skipped -= 1,
            _ => {}
        }
    }
    let id = snapshot.element_id(root).to_owned();
    ir.blocks.push(Block {
        kind: "paragraph".into(),
        align: "unknown".into(),
        text: runs.iter().map(|run| run.text.as_str()).collect(),
        runs,
        source_nodes: vec![id.clone()],
        ..Default::default()
    });
    ir.warnings.push(Diagnostic::new(
        WarningKind::NestingLimit,
        Details {
            source_nodes: vec![id],
            reason: Some("HTML 深度超过 256 或正文块嵌套超过 24 层，已按原顺序提取文本".into()),
            ..Default::default()
        },
    ));
}
