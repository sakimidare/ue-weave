//! 导出格式预设，应用于文章结构的输出副本。
use crate::model::{Block, Ir, Run};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// 保守：尽量保留格式。
    #[default]
    #[value(alias = "保守")]
    Conservative,
    /// 均衡：简化字体和间距。
    #[value(alias = "均衡")]
    Balanced,
    /// 激进：简化格式，方便编辑。
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
    pub repair_corrupt_fonts: bool,
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
    pub trim_leading_spaces: bool,
    pub drop_empty_paragraphs: bool,
    pub native_markdown: bool,
}

#[derive(clap::Args, Default)]
#[command(next_help_heading = "格式调整")]
pub struct Overrides {
    /// 在正文上方显示标题、日期、作者等信息；所有预设默认关闭。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    include_metadata: Option<bool>,
    /// 关闭损坏字体名称修复。
    #[arg(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_repair_corrupt_fonts: bool,
    /// 移除字体名称，可覆盖 --strip-fonts。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_font_family: Option<bool>,
    /// 移除字号，可覆盖 --strip-fonts。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_font_size: Option<bool>,
    /// 移除文字颜色，可覆盖 --strip-colors。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_text_color: Option<bool>,
    /// 移除背景色，可覆盖 --strip-colors。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_background_color: Option<bool>,
    /// 移除行距，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_line_height: Option<bool>,
    /// 移除段间距，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_paragraph_spacing: Option<bool>,
    /// 移除表格、列和单元格固定尺寸，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_table_dimensions: Option<bool>,
    /// 移除单元格内边距，可覆盖 --strip-layout。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_cell_padding: Option<bool>,
    /// 移除图片显式宽高；默认保留。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_image_dimensions: Option<bool>,
    /// 移除字体和字号声明。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_fonts: Option<bool>,
    /// 移除文字颜色和背景色。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_colors: Option<bool>,
    /// 移除行距、段间距、固定表格尺寸和单元格内边距。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_layout: Option<bool>,
    /// 移除显式文本对齐。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_alignment: Option<bool>,
    /// 移除首行缩进声明；保留原文中的空格。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    strip_indent: Option<bool>,
    /// 清理段落、标题和署名开头的排版空格。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    trim_leading_spaces: Option<bool>,
    /// 移除纯空白段落，保留正文内换行。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    drop_empty_paragraphs: Option<bool>,
    /// 简单列表和表格优先使用原生 Markdown。
    #[arg(long, num_args=0..=1, require_equals=true, default_missing_value="true", value_name="BOOL", hide_possible_values=true)]
    native_markdown: Option<bool>,
}

impl Profile {
    pub fn settings(self) -> Settings {
        let mut settings = match self {
            Self::Conservative => Settings::default(),
            Self::Balanced => Settings {
                strip_fonts: true,
                strip_layout: true,
                native_markdown: true,
                ..Settings::default()
            },
            Self::Aggressive => Settings {
                strip_fonts: true,
                strip_colors: true,
                strip_layout: true,
                strip_alignment: true,
                strip_indent: true,
                trim_leading_spaces: true,
                drop_empty_paragraphs: true,
                native_markdown: true,
                ..Settings::default()
            },
        };
        settings.repair_corrupt_fonts = true;
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
            trim_leading_spaces,
            drop_empty_paragraphs,
            native_markdown
        );
        if self.no_repair_corrupt_fonts {
            settings.repair_corrupt_fonts = false;
        }
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
    let mut changes = Vec::new();
    transform(&mut ir.blocks, settings, &mut changes, true);
    diff(
        &json!(source.blocks),
        &json!(ir.blocks),
        "blocks",
        &[],
        &mut changes,
    );
    Output { ir, changes }
}

pub fn prepare_documents(source: &Ir, settings: Settings) -> Output {
    let mut ir = source.clone();
    let mut changes = Vec::new();
    transform(&mut ir.blocks, settings, &mut changes, false);
    Output { ir, changes }
}

fn run(run: &mut Run, settings: Settings, changes: &mut Vec<Value>, record_changes: bool) {
    if settings.repair_corrupt_fonts {
        if let Some(font) = &mut run.font_family {
            let repaired = crate::safety::repair_corrupted_font(font);
            if repaired != *font {
                if record_changes {
                    changes.push(json!({
                        "kind": "corrupt-font-repair",
                        "field": "runs.font_family",
                        "original": font,
                        "value": repaired,
                        "inferred": true,
                        "confidence": "high",
                        "source_nodes": run.source_nodes,
                        "reason": "修复编辑器序列化产生的 undefined 字体名包装"
                    }));
                }
                *font = repaired;
            }
        }
    }
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

fn transform(
    blocks: &mut Vec<Block>,
    settings: Settings,
    changes: &mut Vec<Value>,
    record_changes: bool,
) {
    for block in blocks.iter_mut() {
        if record_changes
            && settings.native_markdown
            && block.kind == "table"
            && crate::markdown::table::header(block) == Some(crate::markdown::table::Header::Empty)
        {
            changes.push(json!({
                "kind": "markdown-table-header", "field": "markdown.header",
                "original": null, "value": "empty", "inferred": false,
                "confidence": "high", "source_nodes": block.source_nodes,
                "reason": "为 GFM 表格添加空表头，原有各行保持为数据行；完整 IR 保留原始结构"
            }));
        }
        if settings.trim_leading_spaces {
            trim_leading_spaces(block, changes, record_changes);
        }
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
            run(item, settings, changes, record_changes);
        }
        if let Some(caption) = &mut block.caption {
            for item in caption {
                run(item, settings, changes, record_changes);
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
        transform(&mut block.children, settings, changes, record_changes);
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
                run(item, settings, changes, record_changes);
            }
            transform(&mut cell.blocks, settings, changes, record_changes);
        }
    }
    if settings.drop_empty_paragraphs {
        blocks.retain(|block| {
            !(block.kind == "paragraph"
                && block.children.is_empty()
                && block.rows.is_empty()
                && block.runs.iter().all(|run| {
                    run.image.is_none()
                        && run.link.is_none()
                        && run.text.chars().all(char::is_whitespace)
                }))
        });
    }
}

fn trim_leading_spaces(block: &mut Block, changes: &mut Vec<Value>, record_changes: bool) {
    if !matches!(block.kind.as_str(), "paragraph" | "heading" | "signature") {
        return;
    }
    if block.runs.iter().any(|run| run.inline_kind.is_some()) {
        return;
    }
    // 保留预格式化内容和行内代码。
    let fragment = scraper::Html::parse_fragment(&block.raw_html);
    if fragment
        .tree
        .nodes()
        .filter_map(scraper::ElementRef::wrap)
        .any(|el| {
            matches!(el.value().name(), "pre" | "code")
                || crate::rules::style(el, "white-space").is_some_and(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "pre" | "pre-wrap" | "break-spaces"
                    )
                })
        })
    {
        return;
    }
    let mut changed = false;
    for (index, run) in block.runs.iter_mut().enumerate() {
        if run.image.is_some() || run.line_break {
            break;
        }
        let trimmed = run
            .text
            .trim_start_matches([' ', '\t', '\u{a0}', '\u{3000}']);
        let removed = run.text.len() - trimmed.len();
        if removed > 0 {
            if record_changes {
                changes.push(json!({
                    "kind": "leading-spaces",
                    "field": format!("runs[{index}].text"),
                    "original": run.text,
                    "value": trimmed,
                    "removed_prefix": &run.text[..removed],
                    "source_nodes": block.source_nodes,
                    "confidence": "high",
                    "inferred": false,
                    "reason": "按 trim-leading-spaces 开关清理段首排版空白"
                }));
            }
            run.text.drain(..removed);
            changed = true;
        }
        if !run.text.is_empty() {
            break;
        }
    }
    if changed {
        block.text = block.runs.iter().map(|run| run.text.as_str()).collect();
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
                diff(
                    value,
                    b.get(key).unwrap_or(&Value::Null),
                    &format!("{path}.{key}"),
                    sources,
                    out,
                );
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (old, new)) in a.iter().zip(b).enumerate() {
                diff(old, new, &format!("{path}[{index}]"), sources, out);
            }
        }
        _ => out.push(json!({
            "field": path,
            "original": before,
            "value": after,
            "source_nodes": sources,
            "confidence": "high",
            "inferred": false,
            "reason": "按输出预设和开关转换格式",
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leading_spaces_are_cleaned_across_runs_only_in_output() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><p> &nbsp;<span>　\t</span><b> 标题</b> 中间&nbsp;空格<br>  下一行</p><h2>\
            &nbsp;标题</h2><p style='text-align:right'>　教务处</p><pre>  code\n  next</pre><p \
            style='white-space:pre-wrap'>  保留</p><p>  <code>code</code></p><p><img src='a.png'>  图片后</p></div>");
        let source = crate::parser::parse(&document, ".Article_Content", true, None).unwrap();
        let snapshot = serde_json::to_string(&source).unwrap();
        let output = prepare(&source, Profile::Aggressive.settings());
        let documents_only = prepare_documents(&source, Profile::Aggressive.settings());
        assert!(documents_only.changes.is_empty());
        assert_eq!(json!(documents_only.ir), json!(output.ir));
        assert_eq!(output.ir.blocks[0].text, "标题 中间\u{a0}空格\n  下一行");
        assert_eq!(output.ir.blocks[1].text, "标题");
        assert_eq!(output.ir.blocks[2].text, "教务处");
        for index in 3..source.blocks.len() {
            assert_eq!(output.ir.blocks[index].text, source.blocks[index].text);
        }
        assert!(output.changes.iter().any(|change| {
            change["field"] == "blocks[0].runs[0].text"
                && change["original"] == " \u{a0}"
                && change["value"] == ""
                && change["source_nodes"]
                    .as_array()
                    .is_some_and(|nodes| !nodes.is_empty())
        }));
        assert_eq!(serde_json::to_string(&source).unwrap(), snapshot);
        for profile in [Profile::Conservative, Profile::Balanced] {
            assert_eq!(
                prepare(&source, profile.settings()).ir.blocks[0].text,
                source.blocks[0].text
            );
        }
        let settings = Overrides {
            trim_leading_spaces: Some(false),
            ..Overrides::default()
        }
        .resolve(Profile::Aggressive);
        assert_eq!(
            prepare(&source, settings).ir.blocks[0].text,
            source.blocks[0].text
        );
    }

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
        let document = scraper::Html::parse_document(
            "<div class='Article_Content'><p style='line-height:2;margin-top:8px'><span \
                style='font-family:Example;font-size:20px;color:red;background-color:yellow'>正文</span><img \
                src='photo.png' width='100' height='80'></p></div>",
        );
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
        let document = scraper::Html::parse_document("<div class='Article_Content'><p style='text-align:center;text-indent:2em;line-height:2;\
            margin-top:8px'><span style='font-family:Example;font-size:20px;color:red;\
            background-color:yellow'><b><u>正文</u></b></span><a href='https://example.org'>链接</a><img \
            src='photo.png'></p><p><br></p><table width='500'><tr><td style='color:red;width:300px'><i>\
            单元格</i></td></tr></table></div>");
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

    #[test]
    fn corrupt_font_repair_is_enabled_by_default_and_can_be_disabled() {
        let document = scraper::Html::parse_document(
            "<div class='Article_Content'><p><span style='font-family:微软雅黑, undefinedundefinedmicrosoft yaheiundefinedundefined'>文字</span></p></div>",
        );
        let source = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        for profile in [
            Profile::Conservative,
            Profile::Balanced,
            Profile::Aggressive,
        ] {
            let output = prepare(&source, profile.settings());
            let mut fonts = Vec::new();
            for block in &output.ir.blocks {
                let mut block = block.clone();
                block.visit_runs_mut(&mut |run| {
                    if let Some(font) = &run.font_family {
                        fonts.push(font.clone());
                    }
                });
            }
            assert!(output
                .changes
                .iter()
                .any(|change| change["kind"] == "corrupt-font-repair"));
            if matches!(profile, Profile::Conservative) {
                assert_eq!(
                    fonts.first().map(String::as_str),
                    Some("微软雅黑, \"microsoft yahei\"")
                );
            }
        }
        let options = Overrides {
            no_repair_corrupt_fonts: true,
            ..Default::default()
        };
        let output = prepare_documents(&source, options.resolve(Profile::Conservative));
        for rendered in [
            crate::renderer::render(&output.ir),
            crate::markdown::render(&output.ir),
            crate::typst::render(&output.ir),
        ] {
            assert!(
                rendered.contains("undefinedundefinedmicrosoft yaheiundefinedundefined"),
                "{rendered}"
            );
        }
        let mut fonts = Vec::new();
        for block in &output.ir.blocks {
            let mut block = block.clone();
            block.visit_runs_mut(&mut |run| {
                if let Some(font) = &run.font_family {
                    fonts.push(font.clone());
                }
            });
        }
        assert!(fonts
            .iter()
            .any(|font| font == "微软雅黑, undefinedundefinedmicrosoft yaheiundefinedundefined"));
        assert!(!output
            .changes
            .iter()
            .any(|change| change["kind"] == "corrupt-font-repair"));
    }
}
