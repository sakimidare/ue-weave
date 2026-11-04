use super::context::Context;
use crate::model::{Block, Ir, Run};
use std::{collections::HashMap, ops::Range};

pub(super) struct OutputContext {
    paragraphs: Vec<String>,
    references: HashMap<String, Vec<(usize, Range<usize>)>>,
}

impl OutputContext {
    pub(super) fn new(ir: &Ir, context: &Context) -> Self {
        let mut result = Self {
            paragraphs: Vec::new(),
            references: HashMap::new(),
        };
        let mut pending: Vec<_> = ir.blocks.iter().rev().collect();
        while let Some(block) = pending.pop() {
            result.block(block, context);
            pending.extend(block.children.iter().rev());
            for row in &block.rows {
                for cell in row {
                    result.runs(&cell.runs, context);
                    pending.extend(cell.blocks.iter().rev());
                }
            }
        }
        result
    }

    fn block(&mut self, block: &Block, context: &Context) {
        if let Some(caption) = &block.caption {
            self.runs(caption, context);
        }
        self.runs(&block.runs, context);
        if block.kind == "code_block" && block.runs.is_empty() && !block.text.is_empty() {
            let i = self.paragraphs.len();
            self.paragraphs.push(block.text.clone());
            for id in &block.source_nodes {
                self.references
                    .entry(id.clone())
                    .or_default()
                    .push((i, 0..block.text.len()));
            }
        }
    }

    fn runs(&mut self, runs: &[Run], context: &Context) {
        let mut text = String::new();
        let i = self.paragraphs.len();
        for run in runs {
            let start = text.len();
            // 在终端片段中将控制字符转换成可见表示。
            text.extend(run.text.chars().map(|c| {
                if c.is_control() && c != '\n' && c != '\t' {
                    '�'
                } else {
                    c
                }
            }));
            if run.image.is_some() {
                text.push_str("[图片]");
            }
            if start == text.len() {
                continue;
            }
            for id in &run.source_nodes {
                for ancestor in context.ancestors(id) {
                    let entries = self.references.entry(ancestor).or_default();
                    if let Some((_, range)) = entries
                        .last_mut()
                        .filter(|(p, r)| *p == i && r.end == start)
                    {
                        range.end = text.len();
                    } else {
                        entries.push((i, start..text.len()));
                    }
                }
            }
        }
        if !text.is_empty() {
            self.paragraphs.push(text);
        }
    }

    pub(super) fn locate(&self, id: &str) -> Vec<(&str, Range<usize>)> {
        self.references
            .get(id)
            .into_iter()
            .flatten()
            .map(|(i, range)| (self.paragraphs[*i].as_str(), range.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_ranges_follow_cleaned_runs_and_preserve_node_identity() {
        let html = scraper::Html::parse_document("<div class='Article_Content'><p>前<b>undefinedundefined保留</b>后</p><p>保留</p><p><span>undefinedundefined</span></p></div>");
        let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
        let context = Context::new(&ir.original_nodes);
        let output = OutputContext::new(&ir, &context);
        let bold = ir.original_nodes.iter().find(|n| n["tag"] == "b").unwrap()["id"]
            .as_str()
            .unwrap();
        let locations = output.locate(bold);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].0, "前保留后");
        assert_eq!(&locations[0].0[locations[0].1.clone()], "保留");
        let removed = ir
            .original_nodes
            .iter()
            .find(|n| n["tag"] == "span")
            .unwrap()["id"]
            .as_str()
            .unwrap();
        assert!(output.locate(removed).is_empty());
    }
}
