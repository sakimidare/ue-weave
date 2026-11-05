use serde::{Deserialize, Serialize};

/// 记录格式值及其判断依据，供内部分析和诊断使用。
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Evidence {
    pub value: serde_json::Value,
    pub confidence: String,
    pub reason: String,
    pub source_nodes: Vec<String>,
}

/// 同一段内格式一致的文字、换行或图片。
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Run {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_kind: Option<InlineKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_nodes: Vec<String>,
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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InlineKind {
    Code,
    Keyboard,
    Sample,
    Variable,
}

impl InlineKind {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Keyboard => "kbd",
            Self::Sample => "samp",
            Self::Variable => "var",
        }
    }
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "code" => Some(Self::Code),
            "kbd" => Some(Self::Keyboard),
            "samp" => Some(Self::Sample),
            "var" => Some(Self::Variable),
            _ => None,
        }
    }
}

/// 图片的替代文字、说明及显式宽高。
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ImageAttributes {
    pub alt: Option<String>,
    pub title: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
}

/// 段落、标题、列表或表格等正文块。
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct Block {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_language: Option<String>,
    // 基础结构
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,

    // 文字与段落格式
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default = "unknown")]
    pub align: String,
    #[serde(default)]
    pub indent: Evidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<Evidence>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub spacing: std::collections::BTreeMap<String, Evidence>,

    // 列表
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<ListAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_value: Option<i64>,

    // 表格
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<Run>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<Vec<TableCell>>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub table_styles: std::collections::BTreeMap<String, Evidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_groups: Vec<TableColumn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_groups: Vec<Option<RowGroup>>,

    // 推断记录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<Evidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_structure: Option<Evidence>,

    // HTML 来源
    #[serde(default)]
    pub raw_html: String,
    #[serde(default)]
    pub source_nodes: Vec<String>,
}

/// 表格单元格及其合并范围、格式和内容。
#[derive(Serialize, Deserialize, Clone)]
pub struct TableCell {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub styles: std::collections::BTreeMap<String, Evidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub header: bool,
    #[serde(default = "one")]
    pub colspan: usize,
    #[serde(default = "one")]
    pub rowspan: usize,
    #[serde(default)]
    pub runs: Vec<Run>,
}

/// 表格中的表头、表体或表尾分组。
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RowGroup {
    pub tag: String,
    pub source_node: String,
}

/// 表格列或列组的跨度与宽度。
#[derive(Serialize, Deserialize, Clone)]
pub struct TableColumn {
    pub span: usize,
    pub width: Option<Evidence>,
    pub children: Vec<TableColumn>,
    pub source_nodes: Vec<String>,
}

/// 列表的编号方式、起始序号及自定义标记。
#[derive(Serialize, Deserialize, Clone)]
pub struct ListAttributes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_style: Option<Evidence>,
    #[serde(default)]
    pub ordered: bool,
    pub start: Option<i64>,
    #[serde(default)]
    pub reversed: bool,
    pub style: Option<String>,
}

/// 内部文章分析数据，包含正文、资源、来源记录和诊断。
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct Ir {
    pub schema: crate::ir_format::Schema,
    pub tool: crate::ir_format::Tool,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub attachments: Vec<serde_json::Value>,
    #[serde(default)]
    pub images: Vec<serde_json::Value>,
    #[serde(default)]
    pub warnings: Vec<crate::diagnostic::Diagnostic>,
    #[serde(default)]
    pub original_nodes: Vec<serde_json::Value>,
    pub source: Option<String>,
}

fn unknown() -> String {
    "unknown".into()
}
fn one() -> usize {
    1
}

impl Block {
    /// 统计本块及嵌套子块和单元格中的正文块数量。
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
    /// 遍历正文、题注和单元格中的全部文字片段并原地修改。
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
    /// 返回 1–6 范围的标题级别，缺省为二级标题。
    pub fn heading_level(&self) -> usize {
        self.level
            .as_ref()
            .and_then(|e| e.value.as_u64())
            .unwrap_or(2)
            .clamp(1, 6) as usize
    }
}
