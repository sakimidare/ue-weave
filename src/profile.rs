//! Output-only policies. Canonical IR and source snapshots remain editable evidence.
use crate::model::{Block, Ir, Run};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    #[default]
    #[value(alias = "保守")]
    Conservative,
    #[value(alias = "均衡")]
    Balanced,
    #[value(alias = "激进")]
    Aggressive,
}

pub struct Output {
    pub ir: Ir,
    pub changes: Vec<Value>,
}

#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct Settings {
    pub include_metadata: bool,
    pub strip_font_family: bool,
    pub strip_font_size: bool,
    pub strip_text_color: bool,
    pub strip_background_color: bool,
    pub strip_line_height: bool,
    pub strip_paragraph_spacing: bool,
    pub strip_table_dimensions: bool,
    pub strip_cell_padding: bool,
    pub strip_image_dimensions: bool,
    pub strip_fonts: bool,
    pub strip_colors: bool,
    pub strip_layout: bool,
    pub strip_alignment: bool,
    pub strip_indent: bool,
    pub drop_empty_paragraphs: bool,
    pub native_markdown: bool,
}

#[derive(clap::Args, Default)]
pub struct Overrides {
    /// 在 HTML、Markdown 和 Typst 正文上方显示标题、日期、作者等元数据。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    include_metadata: Option<bool>,
    /// 移除字体名称，可覆盖 --strip-fonts。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_font_family: Option<bool>,
    /// 移除字号，可覆盖 --strip-fonts。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_font_size: Option<bool>,
    /// 移除文字颜色，可覆盖 --strip-colors。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_text_color: Option<bool>,
    /// 移除背景色，可覆盖 --strip-colors。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_background_color: Option<bool>,
    /// 移除行距，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_line_height: Option<bool>,
    /// 移除段间距，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_paragraph_spacing: Option<bool>,
    /// 移除表格、列和单元格固定尺寸，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_table_dimensions: Option<bool>,
    /// 移除单元格内边距，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_cell_padding: Option<bool>,
    /// 移除图片显式宽高；默认保留。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_image_dimensions: Option<bool>,
    /// 移除字体和字号声明。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_fonts: Option<bool>,
    /// 移除文字颜色和背景色。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_colors: Option<bool>,
    /// 移除行距、段间距、固定表格尺寸和单元格内边距。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_layout: Option<bool>,
    /// 移除显式文本对齐。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_alignment: Option<bool>,
    /// 移除首行缩进声明；保留原文中的空格。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    strip_indent: Option<bool>,
    /// 移除纯空白段落，保留正文内换行。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    drop_empty_paragraphs: Option<bool>,
    /// 简单列表和表格优先使用原生 Markdown。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true")]
    native_markdown: Option<bool>,
}

impl Profile {
    pub fn settings(self) -> Settings {
        let mut settings = match self {
            Self::Conservative => Settings::default(),
            Self::Balanced => Settings {
                strip_fonts: true,
                strip_layout: true,
                ..Settings::default()
            },
            Self::Aggressive => Settings {
                strip_fonts: true,
                strip_colors: true,
                strip_layout: true,
                strip_alignment: true,
                strip_indent: true,
                drop_empty_paragraphs: true,
                native_markdown: true,
                ..Settings::default()
            },
        };
        settings.expand_groups();
        settings
    }
}

impl Settings {
    fn expand_groups(&mut self) {
        self.strip_font_family = self.strip_fonts;
        self.strip_font_size = self.strip_fonts;
        self.strip_text_color = self.strip_colors;
        self.strip_background_color = self.strip_colors;
        self.strip_line_height = self.strip_layout;
        self.strip_paragraph_spacing = self.strip_layout;
        self.strip_table_dimensions = self.strip_layout;
        self.strip_cell_padding = self.strip_layout;
    }
}

impl Overrides {
    pub fn resolve(&self, profile: Profile) -> Settings {
        let mut settings = profile.settings();
        macro_rules! override_fields { ($($field:ident),*) => { $(if let Some(value) = self.$field { settings.$field = value; })* }; }
        override_fields!(
            include_metadata,
            strip_fonts,
            strip_colors,
            strip_layout,
            strip_alignment,
            strip_indent,
            drop_empty_paragraphs,
            native_markdown
        );
        settings.expand_groups();
        override_fields!(
            strip_font_family,
            strip_font_size,
            strip_text_color,
            strip_background_color,
            strip_line_height,
            strip_paragraph_spacing,
            strip_table_dimensions,
            strip_cell_padding,
            strip_image_dimensions
        );
        settings
    }
}

pub fn prepare(source: &Ir, settings: Settings) -> Output {
    let mut ir = source.clone();
    transform(&mut ir.blocks, settings);
    let mut changes = Vec::new();
    diff(
        &json!(source.blocks),
        &json!(ir.blocks),
        "blocks",
        &[],
        &mut changes,
    );
    Output { ir, changes }
}

fn run(run: &mut Run, settings: Settings) {
    if settings.strip_font_family {
        run.font_family = None;
    }
    if settings.strip_font_size {
        run.font_size = None;
    }
    if settings.strip_text_color {
        run.color = None;
    }
    if settings.strip_background_color {
        run.background_color = None;
    }
    if settings.strip_image_dimensions {
        if let Some(attributes) = &mut run.image_attributes {
            attributes.width = None;
            attributes.height = None;
        }
    }
}

fn transform(blocks: &mut Vec<Block>, settings: Settings) {
    for block in blocks.iter_mut() {
        if settings.strip_line_height {
            block.line_height = None;
        }
        if settings.strip_paragraph_spacing {
            block.spacing.clear();
        }
        if settings.strip_table_dimensions {
            for property in [
                "width",
                "height",
                "table-layout",
                "margin-left",
                "margin-right",
            ] {
                block.table_styles.remove(property);
            }
            for group in &mut block.column_groups {
                group.width = None;
                for col in &mut group.children {
                    col.width = None;
                }
            }
        }
        for item in &mut block.runs {
            run(item, settings);
        }
        if let Some(caption) = &mut block.caption {
            for item in caption {
                run(item, settings);
            }
        }
        if settings.strip_indent {
            block.indent.value = json!("unknown");
        }
        if settings.strip_alignment {
            block.align = "unknown".into();
        }
        if settings.strip_background_color {
            block.table_styles.remove("background-color");
        }
        transform(&mut block.children, settings);
        for cell in block.rows.iter_mut().flatten() {
            if settings.strip_table_dimensions {
                cell.styles.remove("width");
                cell.styles.remove("height");
            }
            if settings.strip_cell_padding {
                for property in [
                    "padding",
                    "padding-top",
                    "padding-bottom",
                    "padding-left",
                    "padding-right",
                ] {
                    cell.styles.remove(property);
                }
            }
            if settings.strip_background_color {
                cell.styles.remove("background-color");
            }
            if settings.strip_alignment {
                cell.styles.remove("text-align");
                cell.styles.remove("vertical-align");
            }
            for item in &mut cell.runs {
                run(item, settings);
            }
            transform(&mut cell.blocks, settings);
        }
    }
    if settings.drop_empty_paragraphs {
        blocks.retain(|block| {
            !(block.kind == "paragraph"
                && block.children.is_empty()
                && block.rows.is_empty()
                && block
                    .runs
                    .iter()
                    .all(|run| run.image.is_none() && run.text.chars().all(char::is_whitespace)))
        });
    }
}

fn diff(before: &Value, after: &Value, path: &str, sources: &[Value], out: &mut Vec<Value>) {
    if before == after {
        return;
    }
    let sources = before
        .get("source_nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(sources);
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                diff(value, b.get(key).unwrap_or(&Value::Null), &format!("{path}.{key}"), sources, out);
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (old, new)) in a.iter().zip(b).enumerate() {
                diff(old, new, &format!("{path}[{index}]"), sources, out);
            }
        }
        _ => out.push(json!({"field":path,"original":before,"value":after,"source_nodes":sources,"confidence":"high","inferred":false,"reason":"按输出预设和开关转换格式"})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn granular_overrides_take_precedence_and_preserve_other_styles() {
        let options = Overrides {
            strip_fonts: Some(true),
            strip_font_size: Some(false),
            strip_text_color: Some(false),
            strip_line_height: Some(false),
            strip_image_dimensions: Some(true),
            ..Overrides::default()
        };
        let settings = options.resolve(Profile::Aggressive);
        assert!(settings.strip_font_family);
        assert!(!settings.strip_font_size);
        let document = scraper::Html::parse_document("<div class='Article_Content'><p style='line-height:2;margin-top:8px'><span style='font-family:Example;font-size:20px;color:red;background-color:yellow'>正文</span><img src='photo.png' width='100' height='80'></p></div>");
        let source = crate::parser::parse(&document, ".Article_Content", true, None).unwrap();
        let output = prepare(&source, settings);
        let html = crate::renderer::render(&output.ir);
        assert!(html.contains("font-size:20px"));
        assert!(html.contains("color:red"));
        assert!(html.contains("line-height:2"));
        for removed in [
            "font-family",
            "background-color",
            "margin-top",
            "width=",
            "height=",
        ] {
            assert!(!html.contains(removed), "{removed}: {html}");
        }
        assert!(crate::renderer::render(&source).contains("font-family"));
    }

    #[test]
    fn profiles_preserve_semantics_and_canonical_evidence() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><p style='text-align:center;text-indent:2em;line-height:2;margin-top:8px'><span style='font-family:Example;font-size:20px;color:red;background-color:yellow'><b><u>正文</u></b></span><a href='https://example.org'>链接</a><img src='photo.png'></p><p><br></p><table width='500'><tr><td style='color:red;width:300px'><i>单元格</i></td></tr></table></div>");
        let source = crate::parser::parse(&document, ".Article_Content", true, None).unwrap();
        let snapshot = serde_json::to_string(&source).unwrap();
        let conservative = prepare(&source, Profile::Conservative.settings());
        assert!(conservative.changes.is_empty());
        assert_eq!(
            crate::renderer::render(&source),
            crate::renderer::render(&conservative.ir)
        );
        let balanced = prepare(&source, Profile::Balanced.settings());
        let html = crate::renderer::render(&balanced.ir);
        assert!(
            !html.contains("font-family")
                && !html.contains("font-size")
                && !html.contains("line-height")
        );
        assert!(html.contains("color:red") && html.contains("text-align:center"));
        let aggressive = prepare(&source, Profile::Aggressive.settings());
        let html = crate::renderer::render(&aggressive.ir);
        assert!(
            !html.contains("color:")
                && !html.contains("text-align:center")
                && !html.contains("<br>")
        );
        for output in [&balanced, &aggressive] {
            let html = crate::renderer::render(&output.ir);
            for retained in [
                "正文",
                "链接",
                "photo.png",
                "<table",
                "单元格",
                "<strong>",
                "<u>",
            ] {
                assert!(html.contains(retained), "{retained}");
            }
            assert!(!output.changes.is_empty());
        }
        assert_eq!(serde_json::to_string(&source).unwrap(), snapshot);
    }
}
