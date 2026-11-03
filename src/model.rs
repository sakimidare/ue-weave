use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Evidence {
    pub value: serde_json::Value,
    pub confidence: String,
    pub reason: String,
    pub source_nodes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Run {
    #[serde(default)]
    pub line_break: bool,
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_attributes: Option<ImageAttributes>,
    pub source: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ImageAttributes {
    pub alt: Option<String>,
    pub title: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Block {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_structure: Option<Evidence>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub table_styles: std::collections::BTreeMap<String, Evidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_groups: Vec<TableColumn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_groups: Vec<Option<RowGroup>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_value: Option<i64>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub spacing: std::collections::BTreeMap<String, Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<Run>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<ListAttributes>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<Vec<TableCell>>,
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub indent: Evidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<Evidence>,
    pub align: String,
    pub runs: Vec<Run>,
    pub raw_html: String,
    pub source_nodes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<Evidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<Evidence>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TableCell {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub styles: std::collections::BTreeMap<String, Evidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
    pub header: bool,
    pub colspan: usize,
    pub rowspan: usize,
    pub runs: Vec<Run>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RowGroup {
    pub tag: String,
    pub source_node: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TableColumn {
    pub span: usize,
    pub width: Option<Evidence>,
    pub children: Vec<TableColumn>,
    pub source_nodes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ListAttributes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_style: Option<Evidence>,
    pub ordered: bool,
    pub start: Option<i64>,
    pub reversed: bool,
    pub style: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Ir {
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub blocks: Vec<Block>,
    pub attachments: Vec<serde_json::Value>,
    pub images: Vec<serde_json::Value>,
    pub warnings: Vec<serde_json::Value>,
    pub original_nodes: Vec<serde_json::Value>,
    pub source: Option<String>,
}

impl Block {
    pub fn block_count(&self) -> usize {
        1 + self.children.iter().map(Self::block_count).sum::<usize>()
            + self
                .rows
                .iter()
                .flatten()
                .flat_map(|cell| &cell.blocks)
                .map(Self::block_count)
                .sum::<usize>()
    }
    pub fn visit_runs_mut(&mut self, visitor: &mut impl FnMut(&mut Run)) {
        if let Some(caption) = &mut self.caption {
            for run in caption {
                visitor(run);
            }
        }
        for child in &mut self.children {
            child.visit_runs_mut(visitor);
        }
        for run in &mut self.runs {
            visitor(run);
        }
        for cell in self.rows.iter_mut().flatten() {
            for run in &mut cell.runs {
                visitor(run);
            }
            for block in &mut cell.blocks {
                block.visit_runs_mut(visitor);
            }
        }
    }
    pub fn heading_level(&self) -> usize {
        self.level
            .as_ref()
            .and_then(|e| e.value.as_u64())
            .unwrap_or(2)
            .clamp(1, 6) as usize
    }
}
