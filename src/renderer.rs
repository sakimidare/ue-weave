use crate::model::{Block, Ir, Run};

pub(crate) fn run_html(run: &Run) -> String {
    if run.line_break {
        return "<br>".into();
    }
    if let Some(src) = &run.image {
        if !crate::safety::safe_url(src, true) {
            return String::new();
        }
        let mut image = format!(
            "<img src=\"{}\"",
            html_escape::encode_double_quoted_attribute(src)
        );
        if let Some(attrs) = &run.image_attributes {
            let mut dimensions = Vec::new();
            for (name, value) in [
                ("alt", &attrs.alt),
                ("title", &attrs.title),
                ("width", &attrs.width),
                ("height", &attrs.height),
            ] {
                if let Some(value) = value {
                    if matches!(name, "width" | "height") && value.parse::<u32>().is_err() {
                        if crate::style_registry::value(name, value) {
                            dimensions.push(format!("{name}:{value}"));
                        }
                        continue;
                    }
                    image.push_str(&format!(
                        " {name}=\"{}\"",
                        html_escape::encode_double_quoted_attribute(value)
                    ));
                }
            }
            if !dimensions.is_empty() {
                image.push_str(&format!(
                    " style=\"{}\"",
                    html_escape::encode_double_quoted_attribute(&dimensions.join(";"))
                ));
            }
        }
        image.push_str(" />");
        if let Some(link) = &run.link {
            if crate::safety::safe_url(link, false) {
                image = format!(
                    "<a href=\"{}\">{image}</a>",
                    html_escape::encode_double_quoted_attribute(link)
                );
            }
        }
        return image;
    }
    let mut value = html_escape::encode_text(&run.text).to_string();
    if let Some(align) = &run.vertical_align {
        if align == "super" {
            value = format!("<sup>{value}</sup>");
        }
        if align == "sub" {
            value = format!("<sub>{value}</sub>");
        }
    }
    if run.strike {
        value = format!("<s>{value}</s>");
    }
    if run.underline {
        value = format!("<u>{value}</u>");
    }
    if run.italic {
        value = format!("<em>{value}</em>");
    }
    if run.bold {
        value = format!("<strong>{value}</strong>");
    }
    let mut styles = Vec::new();
    if let Some(value) = &run.font_family {
        styles.push(format!("font-family:{value}"));
    }
    if let Some(value) = &run.font_size {
        styles.push(format!("font-size:{value}"));
    }
    if let Some(value) = &run.color {
        styles.push(format!("color:{value}"));
    }
    if let Some(value) = &run.background_color {
        styles.push(format!("background-color:{value}"));
    }
    styles.retain(|declaration| {
        declaration
            .split_once(':')
            .is_some_and(|(property, value)| crate::style_registry::value(property, value))
    });
    if !styles.is_empty() {
        value = format!(
            "<span style=\"{}\">{value}</span>",
            html_escape::encode_double_quoted_attribute(&styles.join(";"))
        );
    }
    if let Some(href) = &run.link {
        if !crate::safety::safe_url(href, false) {
            return value;
        }
        value = format!(
            "<a href=\"{}\">{value}</a>",
            html_escape::encode_double_quoted_attribute(href)
        );
    }
    value
}

pub(crate) fn block_html(block: &Block) -> String {
    match block.kind.as_str() {
        "horizontal_rule" => return "<hr>".into(),
        "code_block" => {
            return format!(
                "<pre><code>{}</code></pre>",
                html_escape::encode_text(&block.text)
            )
        }
        "quote" => {
            return format!(
                "<blockquote>{}</blockquote>",
                block.children.iter().map(block_html).collect::<String>()
            )
        }
        _ => {}
    }
    if block.kind == "list" {
        let ordered = block.list.as_ref().is_some_and(|list| list.ordered);
        let tag = if ordered { "ol" } else { "ul" };
        let mut attrs = String::new();
        if let Some(list) = &block.list {
            if let Some(start) = list.start {
                attrs.push_str(&format!(" start=\"{start}\""));
            }
            if list.reversed {
                attrs.push_str(" reversed");
            }
            if let Some(style) = &list.style {
                if crate::style_registry::value("list-style-type", style) {
                    attrs.push_str(&format!(
                        " style=\"list-style-type:{}\"",
                        html_escape::encode_double_quoted_attribute(style)
                    ));
                }
            }
        }
        return format!(
            "<{tag}{attrs}>{}</{tag}>",
            block.children.iter().map(block_html).collect::<String>()
        );
    }
    if block.kind == "list_item" {
        return format!(
            "<li{}>{}{}</li>",
            block
                .list_value
                .map(|value| format!(" value=\"{value}\""))
                .unwrap_or_default(),
            block.runs.iter().map(run_html).collect::<String>(),
            block.children.iter().map(block_html).collect::<String>()
        );
    }
    if block.kind == "table" {
        let declarations = block
            .table_styles
            .iter()
            .filter_map(|(key, evidence)| {
                let value = evidence.value.as_str()?;
                (crate::style_registry::TABLE_STYLES.contains(&key.as_str())
                    && crate::style_registry::value(key, value))
                .then(|| format!("{key}:{value}"))
            })
            .collect::<Vec<_>>()
            .join(";");
        let mut html = if declarations.is_empty() {
            String::from("<table>")
        } else {
            format!(
                "<table style=\"{}\">",
                html_escape::encode_double_quoted_attribute(&declarations)
            )
        };
        if let Some(caption) = &block.caption {
            html.push_str(&format!(
                "<caption>{}</caption>",
                caption.iter().map(run_html).collect::<String>()
            ));
        }
        let mut previous_group: Option<&crate::model::RowGroup> = None;
        html.push_str(&crate::table::columns_html(&block.column_groups));
        for (index, row) in block.rows.iter().enumerate() {
            let group = block
                .row_groups
                .get(index)
                .and_then(Option::as_ref)
                .filter(|group| matches!(group.tag.as_str(), "thead" | "tbody" | "tfoot"));
            if group != previous_group {
                if let Some(previous) = previous_group {
                    html.push_str(&format!("</{}>", previous.tag));
                }
                if let Some(current) = group {
                    html.push_str(&format!("<{}>", current.tag));
                }
                previous_group = group;
            }
            html.push_str("<tr>");
            for cell in row {
                let tag = if cell.header { "th" } else { "td" };
                let declarations = cell
                    .styles
                    .iter()
                    .filter_map(|(property, evidence)| {
                        let value = evidence.value.as_str()?;
                        (crate::style_registry::CELL_STYLES.contains(&property.as_str())
                            && crate::style_registry::value(property, value))
                        .then(|| format!("{property}:{value}"))
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                let style = if declarations.is_empty() {
                    String::new()
                } else {
                    format!(
                        " style=\"{}\"",
                        html_escape::encode_double_quoted_attribute(&declarations)
                    )
                };
                html.push_str(&format!(
                    "<{tag} colspan=\"{}\" rowspan=\"{}\"{style}>{}</{tag}>",
                    cell.colspan,
                    cell.rowspan,
                    cell.runs.iter().map(run_html).collect::<String>()
                        + &cell.blocks.iter().map(block_html).collect::<String>()
                ));
            }
            html.push_str("</tr>");
        }
        if let Some(previous) = previous_group {
            html.push_str(&format!("</{}>", previous.tag));
        }
        html.push_str("</table>");
        return html;
    }
    let tag = if block.kind == "heading" {
        format!("h{}", block.heading_level())
    } else {
        "p".into()
    };
    let inferred = if block.indent.value == "likely" {
        " data-inferred=\"text-indent\""
    } else {
        ""
    };
    let mut styles = Vec::new();
    for (property, evidence) in &block.spacing {
        if let Some(value) = evidence.value.as_str() {
            if ["margin-top", "margin-bottom"].contains(&property.as_str())
                && crate::safety::safe_css(value)
            {
                styles.push(format!("{property}:{value}"));
            }
        }
    }
    if ["left", "center", "right", "justify"].contains(&block.align.as_str()) {
        styles.push(format!("text-align:{}", block.align));
    }
    if let Some(indent) = block.indent.value.as_str() {
        if !["likely", "none", "unknown"].contains(&indent) && crate::safety::safe_css(indent) {
            styles.push(format!("text-indent:{indent}"));
        }
    }
    if let Some(height) = block.line_height.as_ref().and_then(|e| e.value.as_str()) {
        if crate::safety::safe_css(height) {
            styles.push(format!("line-height:{height}"));
        }
    }
    let align = if styles.is_empty() {
        String::new()
    } else {
        format!(
            " style=\"{}\"",
            html_escape::encode_double_quoted_attribute(&styles.join(";"))
        )
    };
    format!(
        "<{tag}{inferred}{align}>{}</{tag}>",
        block.runs.iter().map(run_html).collect::<String>()
    )
}

pub fn render(ir: &Ir) -> String {
    let mut output = Vec::new();
    let mut index = 0;
    while index < ir.blocks.len() {
        if ir.blocks[index].kind == "list_item" {
            let mut items = Vec::new();
            while index < ir.blocks.len() && ir.blocks[index].kind == "list_item" {
                items.push(format!(
                    "<li>{}</li>",
                    ir.blocks[index]
                        .runs
                        .iter()
                        .map(run_html)
                        .collect::<String>()
                ));
                index += 1;
            }
            output.push(format!("<ul>{}</ul>", items.join("")));
        } else {
            output.push(block_html(&ir.blocks[index]));
            index += 1;
        }
    }
    output.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Evidence, Run};

    #[test]
    fn renders_semantic_marks_and_inferred_indent() {
        let ir = Ir {
            blocks: vec![Block {
                row_groups: Vec::new(),
                column_groups: Vec::new(),
                table_styles: Default::default(),
                word_structure: None,
                list_value: None,
                children: Vec::new(),
                spacing: Default::default(),
                caption: None,
                list: None,
                line_height: None,
                rows: Vec::new(),
                kind: "heading".into(),
                text: "标题".into(),
                indent: Evidence {
                    value: serde_json::json!("likely"),
                    confidence: "medium".into(),
                    reason: "test".into(),
                    source_nodes: vec!["p".into()],
                },
                align: "center".into(),
                runs: vec![Run {
                    line_break: false,
                    text: "标题".into(),
                    bold: true,
                    italic: false,
                    underline: false,
                    strike: false,
                    font_family: None,
                    font_size: None,
                    color: None,
                    background_color: None,
                    vertical_align: None,
                    link: None,
                    image: None,
                    image_attributes: None,
                    source: "original".into(),
                }],
                raw_html: String::new(),
                source_nodes: vec!["p".into()],
                level: None,
                signature: None,
            }],
            ..Default::default()
        };
        let html = render(&ir);
        assert!(html.contains("<h2 data-inferred=\"text-indent\" style=\"text-align:center\">"));
        assert!(html.contains("<strong>标题</strong>"));
    }
}
