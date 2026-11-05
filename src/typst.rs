use crate::model::{Block, Ir, Run};

fn escape(text: &str) -> String {
    format!("#text({})", quoted(text))
}

pub(crate) fn quoted(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

fn length(value: &str) -> Option<String> {
    for unit in ["px", "pt", "em", "cm", "mm", "in"] {
        if let Some(number) = value.trim().strip_suffix(unit) {
            let number: f64 = number.trim().parse().ok()?;
            if !number.is_finite() || number <= 0.0 {
                return None;
            }
            return Some(if unit == "px" {
                format!("{}pt", number * 0.75)
            } else {
                format!("{number}{unit}")
            });
        }
    }
    None
}

fn color(value: &str) -> Option<String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if lower == "transparent" {
        return Some("rgb(\"#00000000\")".into());
    }
    if let Some(body) = lower
        .strip_prefix("rgb(")
        .or_else(|| lower.strip_prefix("rgba("))
        .and_then(|body| body.strip_suffix(')'))
    {
        let parts: Vec<_> = body
            .split(|ch: char| ch == ',' || ch == '/' || ch.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .collect();
        if !matches!(parts.len(), 3 | 4) {
            return None;
        }
        let channel = |raw: &str, alpha: bool| -> Option<u8> {
            let value = if let Some(number) = raw.strip_suffix('%') {
                number.parse::<f64>().ok()? / 100.0 * 255.0
            } else {
                raw.parse::<f64>().ok()? * if alpha { 255.0 } else { 1.0 }
            };
            value
                .is_finite()
                .then(|| value.clamp(0.0, 255.0).round() as u8)
        };
        let red = channel(parts[0], false)?;
        let green = channel(parts[1], false)?;
        let blue = channel(parts[2], false)?;
        let alpha = if parts.len() == 4 {
            channel(parts[3], true)?
        } else {
            255
        };
        return Some(format!(
            "rgb(\"#{red:02x}{green:02x}{blue:02x}{alpha:02x}\")"
        ));
    }
    if let Some(hex) = value.strip_prefix('#') {
        if [3, 4, 6, 8].contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(format!("rgb({})", quoted(value)));
        }
    }
    match value.to_ascii_lowercase().as_str() {
        "black" | "white" | "red" | "blue" | "green" | "yellow" | "gray" | "silver" | "orange"
        | "purple" => Some(value.to_ascii_lowercase()),
        _ => None,
    }
}

fn run_typst(run: &Run) -> String {
    if run.line_break {
        return "#linebreak()".into();
    }
    if let Some(src) = &run.image {
        if !crate::safety::safe_url(src, true) {
            return String::new();
        }
        if src.starts_with("http:") || src.starts_with("https:") || src.starts_with("//") {
            return format!("#link({})[图片]", quoted(src));
        }
        let mut options = String::new();
        if let Some(attrs) = &run.image_attributes {
            for (name, value) in [("width", &attrs.width), ("height", &attrs.height)] {
                if let Some(value) = value.as_deref().and_then(image_dimension) {
                    options.push_str(&format!(", {name}: {value}"));
                }
            }
            if let Some(alt) = &attrs.alt {
                options.push_str(&format!(", alt: {}", quoted(alt)));
            }
        }
        let image = format!("#image({}{options})", quoted(src));
        return match run
            .link
            .as_deref()
            .filter(|link| crate::safety::safe_url(link, false))
        {
            Some(link) => format!("#link({})[{image}]", quoted(link)),
            None => image,
        };
    }
    let mut text = match run.inline_kind {
        Some(crate::model::InlineKind::Variable) => format!("#emph[{}]", escape(&run.text)),
        Some(_) => format!("#raw({})", quoted(&run.text)),
        None => escape(&run.text),
    };
    if let Some(href) = &run.link {
        if crate::safety::safe_url(href, false) {
            text = format!("#link({})[{text}]", quoted(href));
        }
    }
    if run.strike {
        text = format!("#strike[{text}]");
    }
    if run.underline {
        text = format!("#underline[{text}]");
    }
    if run.italic {
        text = format!("#emph[{text}]");
    }
    if run.bold {
        text = format!("#strong[{text}]");
    }
    if run.vertical_align.as_deref() == Some("super") {
        text = format!("#super[{text}]");
    }
    if run.vertical_align.as_deref() == Some("sub") {
        text = format!("#sub[{text}]");
    }
    if let Some(size) = run.font_size.as_deref().and_then(length) {
        text = format!("#text(size: {size})[{text}]");
    }
    if let Some(color) = run.color.as_deref().and_then(color) {
        text = format!("#text(fill: {color})[{text}]");
    }
    if let Some(family) = &run.font_family {
        let fonts = font_families(family);
        if !fonts.is_empty() {
            text = format!(
                "#text(font: ({},))[{text}]",
                fonts
                    .iter()
                    .map(|font| quoted(font))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    if let Some(fill) = run.background_color.as_deref().and_then(color) {
        text = format!("#highlight(fill: {fill})[{text}]");
    }
    text
}

fn image_dimension(value: &str) -> Option<String> {
    if let Ok(number) = value.parse::<u32>() {
        return Some(format!("{}pt", number as f64 * 0.75));
    }
    if let Some(percent) = value.strip_suffix('%').and_then(|n| n.parse::<f64>().ok()) {
        return (percent.is_finite() && percent >= 0.0).then(|| format!("{percent}%"));
    }
    length(value)
}

fn font_families(value: &str) -> Vec<String> {
    // 与 HTML 导出一致：被过滤的声明保存在来源记录中。
    if !crate::safety::safe_font_family(value) {
        return Vec::new();
    }
    let mut families = Vec::new();
    let mut name = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            name.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                name.push(ch);
            }
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch == ',' {
            if !name.trim().is_empty() {
                families.push(name.trim().to_string());
            }
            name.clear();
        } else {
            name.push(ch);
        }
    }
    if !name.trim().is_empty() {
        families.push(name.trim().to_string());
    }
    families
}

fn block_typst(block: &Block) -> String {
    let content = block.runs.iter().map(run_typst).collect::<String>();
    match block.kind.as_str() {
        "definition_list" => block
            .children
            .iter()
            .map(block_typst)
            .collect::<Vec<_>>()
            .join("\n\n"),
        "definition_term" | "definition_description" => {
            let text = format!(
                "{content}{}",
                block
                    .children
                    .iter()
                    .map(block_typst)
                    .collect::<Vec<_>>()
                    .join("\n\n")
            );
            if block.kind == "definition_term" {
                format!("#strong[{text}]")
            } else {
                format!("#pad(left: 1em)[{text}]")
            }
        }
        "horizontal_rule" => "#line(length: 100%)".into(),
        "code_block" => {
            let language = block
                .code_language
                .as_deref()
                .filter(|lang| crate::code::valid_language(lang))
                .map(|lang| format!(", lang: {}", quoted(lang)))
                .unwrap_or_default();
            format!("#raw({}, block: true{language})", quoted(&block.text))
        }
        "quote" => format!(
            "#quote(block: true)[{}]",
            block
                .children
                .iter()
                .map(block_typst)
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
        "heading" => format!("{} {content}", "=".repeat(block.heading_level())),
        "list_item" => format!(
            "{content}{}",
            block
                .children
                .iter()
                .map(block_typst)
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
        "list" => {
            let ordered = block.list.as_ref().is_some_and(|list| list.ordered);
            let command = if ordered { "enum" } else { "list" };
            let reversed = block.list.as_ref().is_some_and(|list| list.reversed);
            let start = block
                .list
                .as_ref()
                .and_then(|list| list.start)
                .unwrap_or(if reversed {
                    block.children.len() as i64
                } else {
                    1
                });
            let mut next = start as i128;
            let numbers: Vec<i128> = block
                .children
                .iter()
                .map(|child| {
                    let number = child.list_value.map(i128::from).unwrap_or(next);
                    next = number + if reversed { -1 } else { 1 };
                    number
                })
                .collect();
            if ordered && numbers.iter().any(|number| *number <= 0) {
                let cells = block
                    .children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        let number = numbers[index];
                        format!(
                            "[{}], [{}]",
                            escape(&format!("{number}.")),
                            block_typst(child)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return format!(
                    "#grid(columns: (auto, 1fr), column-gutter: 0.5em, row-gutter: 0.4em, {cells})"
                );
            }
            let mut args = if ordered {
                format!("start: {start}, ")
            } else {
                String::new()
            };
            if ordered {
                let numbering = match block.list.as_ref().and_then(|list| list.style.as_deref()) {
                    Some("lower-alpha" | "lower-latin") => "a.",
                    Some("upper-alpha" | "upper-latin") => "A.",
                    Some("lower-roman") => "i.",
                    Some("upper-roman") => "I.",
                    _ => "1.",
                };
                args.push_str(&format!("numbering: {}, ", quoted(numbering)));
            } else {
                let marker = match block.list.as_ref().and_then(|list| list.style.as_deref()) {
                    Some("circle") => Some("○"),
                    Some("square") => Some("▪"),
                    Some("disc") => Some("•"),
                    Some("none") => Some(""),
                    _ => None,
                };
                if let Some(marker) = marker {
                    args.push_str(&format!("marker: [{}], ", escape(marker)));
                }
            }
            format!(
                "#{command}({args}{})",
                block
                    .children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        if ordered {
                            format!("enum.item({})[{}]", numbers[index], block_typst(child))
                        } else {
                            format!("[{}]", block_typst(child))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        "table" => {
            let table = table_typst(block);
            let left = block
                .table_styles
                .get("margin-left")
                .and_then(|v| v.value.as_str());
            let right = block
                .table_styles
                .get("margin-right")
                .and_then(|v| v.value.as_str());
            match (left, right) {
                (Some("auto"), Some("auto")) => format!("#align(center)[{table}]"),
                (Some("auto"), _) => format!("#align(right)[{table}]"),
                (_, Some("auto")) => format!("#align(left)[{table}]"),
                _ => table,
            }
        }
        _ => paragraph_typst(block, content),
    }
}

fn paragraph_typst(block: &Block, mut content: String) -> String {
    let mut paragraph = Vec::new();
    if let Some(indent) = block.indent.value.as_str().and_then(length) {
        paragraph.push(format!("first-line-indent: (amount: {indent}, all: true)"));
    }
    if block.align == "justify" {
        paragraph.push("justify: true".into());
    }
    if !paragraph.is_empty() {
        content = format!("#par({})[{content}]", paragraph.join(", "));
    }
    match block.align.as_str() {
        "left" | "center" | "right" => content = format!("#align({})[{content}]", block.align),
        _ => {}
    }
    let mut spacing = Vec::new();
    for (property, argument) in [("margin-top", "above"), ("margin-bottom", "below")] {
        if let Some(raw) = block.spacing.get(property).and_then(|e| e.value.as_str()) {
            let value = if matches!(raw, "0" | "0px" | "0pt" | "0em") {
                Some("0pt".into())
            } else {
                length(raw)
            };
            if let Some(value) = value {
                spacing.push(format!("{argument}: {value}"));
            }
        }
    }
    if !spacing.is_empty() {
        content = format!("#block({})[{content}]", spacing.join(", "));
    }
    content
}

fn table_typst(block: &Block) -> String {
    let mut occupied = std::collections::HashSet::new();
    let mut cells = Vec::new();
    let mut columns = 0;
    for (y, row) in block.rows.iter().enumerate() {
        let mut x = 0;
        for cell in row {
            while (x..x + cell.colspan).any(|col| occupied.contains(&(col, y))) {
                x += 1;
            }
            for row in y..y + cell.rowspan {
                for col in x..x + cell.colspan {
                    occupied.insert((col, row));
                }
            }
            let mut content = cell.runs.iter().map(run_typst).collect::<String>();
            content.push_str(
                &cell
                    .blocks
                    .iter()
                    .map(block_typst)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            );
            if cell.header {
                content = format!("#strong[{content}]");
            }
            let property = |name: &str| {
                cell.styles
                    .get(name)
                    .and_then(|evidence| evidence.value.as_str())
            };
            let horizontal = match property("text-align") {
                Some("left") => Some("left"),
                Some("center") => Some("center"),
                Some("right") => Some("right"),
                _ => None,
            };
            let vertical = match property("vertical-align") {
                Some("top") => Some("top"),
                Some("middle") => Some("horizon"),
                Some("bottom") => Some("bottom"),
                _ => None,
            };
            let mut options = String::new();
            match (horizontal, vertical) {
                (Some(h), Some(v)) => options.push_str(&format!(", align: {h} + {v}")),
                (Some(a), None) | (None, Some(a)) => options.push_str(&format!(", align: {a}")),
                _ => {}
            }
            if let Some(fill) = property("background-color").and_then(color) {
                options.push_str(&format!(", fill: {fill}"));
            }
            if let Some(inset) = cell_inset(cell) {
                options.push_str(&format!(", inset: {inset}"));
            }
            cells.push(format!(
                "table.cell(x: {x}, y: {y}, colspan: {}, rowspan: {}{options})[{content}]",
                cell.colspan, cell.rowspan
            ));
            x += cell.colspan;
            columns = columns.max(x);
        }
    }
    let caption = block
        .caption
        .as_ref()
        .map(|runs| runs.iter().map(run_typst).collect::<String>())
        .unwrap_or_default();
    if columns == 0 {
        return caption;
    }
    let mut widths = column_widths(&block.column_groups);
    let columns = if widths.is_empty() {
        columns.to_string()
    } else {
        widths.resize(widths.len().max(columns), "auto".into());
        format!("({},)", widths.join(", "))
    };
    let table = format!("#table(columns: {columns},\n{}\n)", cells.join(",\n"));
    if caption.is_empty() {
        table
    } else {
        format!("{caption}\n\n{table}")
    }
}

fn cell_inset(cell: &crate::model::TableCell) -> Option<String> {
    fn padding(value: &str) -> Option<String> {
        if matches!(value.trim(), "0" | "0px" | "0pt" | "0em") {
            Some("0pt".into())
        } else {
            length(value)
        }
    }
    let default = cell
        .styles
        .get("padding")
        .and_then(|e| e.value.as_str())
        .and_then(padding);
    let values: Vec<_> = ["top", "right", "bottom", "left"]
        .into_iter()
        .map(|side| {
            let value = cell
                .styles
                .get(&format!("padding-{side}"))
                .and_then(|e| e.value.as_str())
                .and_then(padding)
                .or_else(|| default.clone());
            (side, value)
        })
        .collect();
    if values.iter().all(|(_, value)| value.is_none()) {
        return None;
    }
    if values.iter().all(|(_, value)| value == &values[0].1) {
        return values[0].1.clone();
    }
    Some(format!(
        "({})",
        values
            .into_iter()
            .filter_map(|(side, value)| value.map(|value| format!("{side}: {value}")))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn column_widths(groups: &[crate::model::TableColumn]) -> Vec<String> {
    fn width(evidence: Option<&crate::model::Evidence>) -> Option<String> {
        let value = evidence?.value.as_str()?.trim();
        if let Some(number) = value.strip_suffix('%').and_then(|n| n.parse::<f64>().ok()) {
            return (number.is_finite() && (0.0..=100.0).contains(&number))
                .then(|| format!("{number}%"));
        }
        if value == "0" {
            return Some("0pt".into());
        }
        length(value)
    }
    let mut result = Vec::new();
    for group in groups {
        let default = width(group.width.as_ref()).unwrap_or("auto".into());
        if group.children.is_empty() {
            result.extend(std::iter::repeat_n(default, group.span.clamp(1, 1000)));
        } else {
            for column in &group.children {
                let value = width(column.width.as_ref()).unwrap_or_else(|| default.clone());
                result.extend(std::iter::repeat_n(value, column.span.clamp(1, 1000)));
            }
        }
    }
    result
}

pub fn render(ir: &Ir) -> String {
    ir.blocks
        .iter()
        .map(block_typst)
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn zero_and_negative_list_numbers_are_not_clamped() {
        let document =
            scraper::Html::parse_document(include_str!("../fixtures/zero-negative-list.html"));
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let output = super::render(&ir);
        assert_eq!(output.matches("#grid(").count(), 3);
        assert!(output.contains("#text(\"0.\")"));
        assert!(output.contains("#text(\"-1.\")"));
        assert!(output.contains("#text(\"-2.\")"));
        let html = crate::renderer::render(&ir);
        assert!(html.contains("start=\"0\""));
        assert!(html.contains("start=\"-2\""));
    }
    #[test]
    fn reversed_lists_keep_explicit_numbers_and_roman_style() {
        let document =
            scraper::Html::parse_document(include_str!("../fixtures/reversed-list.html"));
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let output = super::render(&ir);
        assert!(output.contains("numbering: \"I.\""));
        assert!(output.contains("enum.item(4)"));
        assert!(output.contains("enum.item(3)"));
        assert!(output.contains("numbering: \"a.\""));
        assert!(!ir.warnings.iter().any(|w| w["kind"] == "export-limitation"));
    }
    #[test]
    fn css_rgb_colors_and_alpha_convert_to_bounded_hex() {
        assert_eq!(
            super::color("rgb(255, 0, 128)").as_deref(),
            Some("rgb(\"#ff0080ff\")")
        );
        assert_eq!(
            super::color("rgba(100%, 0%, 0%, 0.5)").as_deref(),
            Some("rgb(\"#ff000080\")")
        );
        assert_eq!(
            super::color("rgb(0 255 0 / 50%)").as_deref(),
            Some("rgb(\"#00ff0080\")")
        );
        assert!(super::color("rgb(NaN, 0, 0)").is_none());
        assert!(super::color("rgb(1,2)").is_none());
    }
    #[test]
    fn font_names_and_highlights_are_safe_typst_values() {
        assert_eq!(
            super::font_families("'Family, Name', \"Other\""),
            ["Family, Name", "Other"]
        );
        let document = scraper::Html::parse_document("<div class='Article_Content'><p><span style='font-family:DejaVu Sans;background-color:#ffeeaa'>[文字] #</span></p></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let output = super::render(&ir);
        assert!(output.contains("font: (\"DejaVu Sans\",)"));
        assert!(output.contains("#highlight(fill: rgb(\"#ffeeaa\"))"));
        assert!(output.contains("#text(\"[文字] #\")"));
    }

    #[test]
    fn default_profile_repairs_corrupted_fonts_and_keeps_the_snapshot() {
        let document = scraper::Html::parse_document(include_str!("../fixtures/seu-585911.html"));
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        assert!(ir
            .warnings
            .iter()
            .any(|warning| warning["kind"] == "undefinedundefined-style"));
        assert!(serde_json::to_string(&ir.original_nodes)
            .unwrap()
            .contains("undefinedundefined"));
        let prepared = crate::profile::prepare_documents(
            &ir,
            crate::profile::Profile::Conservative.settings(),
        );
        assert!(!super::render(&prepared.ir)
            .to_lowercase()
            .contains("undefinedundefined"));
        assert!(!crate::renderer::render(&prepared.ir)
            .to_lowercase()
            .contains("undefinedundefined"));
    }

    #[test]
    fn wrapped_undefined_font_variants_are_filtered_in_every_export() {
        for source in [
            include_str!("../fixtures/seu-583204.html"),
            include_str!("../fixtures/seu-583286.html"),
        ] {
            let document = scraper::Html::parse_document(source);
            let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
            let prepared = crate::profile::prepare_documents(
                &ir,
                crate::profile::Profile::Conservative.settings(),
            );
            for output in [
                super::render(&prepared.ir),
                crate::renderer::render(&prepared.ir),
                crate::markdown::render(&prepared.ir),
            ] {
                assert!(!output.to_ascii_lowercase().contains("undefinedmicrosoft"));
            }
            assert!(ir
                .warnings
                .iter()
                .any(|w| w["kind"] == "undefinedundefined-style"));
            assert!(serde_json::to_string(&ir.original_nodes)
                .unwrap()
                .contains("undefined"));
        }
        assert!(crate::safety::safe_css("Undefined Sans"));
    }
}
