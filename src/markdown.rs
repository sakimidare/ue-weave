use crate::model::{Block, Ir, Run};
pub(crate) mod table;

pub(crate) fn escape_text(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' | '~' => {
                output.push('\\');
                output.push(ch);
            }
            _ => output.push(ch),
        }
    }
    output
}

fn destination(value: &str) -> String {
    let mut output = String::from("<");
    for ch in value.chars() {
        match ch {
            '<' => output.push_str("%3C"),
            '>' => output.push_str("%3E"),
            ' ' => output.push_str("%20"),
            '\\' => output.push_str("%5C"),
            '&' => output.push_str("&amp;"),
            _ => output.push(ch),
        }
    }
    output.push('>');
    output
}

fn run_markdown(run: &Run) -> String {
    if run.line_break {
        return "<br>\n".into();
    }
    if let Some(src) = &run.image {
        if !crate::safety::safe_url(src, true) {
            return String::new();
        }
        let alt = run
            .image_attributes
            .as_ref()
            .and_then(|attrs| attrs.alt.as_deref())
            .unwrap_or("");
        let image = format!("![{}]({})", escape_text(alt), destination(src));
        return match &run.link {
            Some(href) if crate::safety::safe_url(href, false) => {
                format!("[{image}]({})", destination(href))
            }
            _ => image,
        };
    }
    if run.font_family.is_some()
        || run.font_size.is_some()
        || run.color.is_some()
        || run.background_color.is_some()
    {
        return crate::renderer::run_html(run);
    }
    let mut text = escape_text(&run.text);
    if let Some(href) = &run.link {
        if crate::safety::safe_url(href, false) {
            text = format!("[{text}]({})", destination(href));
        }
    }
    if run.strike {
        text = format!("~~{text}~~");
    }
    if run.underline {
        text = format!("<u>{text}</u>");
    }
    if run.italic {
        text = format!("*{text}*");
    }
    if run.bold {
        text = format!("**{text}**");
    }
    if run.vertical_align.as_deref() == Some("super") {
        text = format!("<sup>{text}</sup>");
    }
    if run.vertical_align.as_deref() == Some("sub") {
        text = format!("<sub>{text}</sub>");
    }
    text
}

fn block_markdown(block: &Block) -> String {
    let content = block.runs.iter().map(run_markdown).collect::<String>();
    match block.kind.as_str() {
        "horizontal_rule" => "---".into(),
        "code_block" => {
            let fence = "`".repeat(
                block
                    .text
                    .split(|ch| ch != '`')
                    .map(str::len)
                    .max()
                    .unwrap_or(0)
                    .max(2)
                    + 1,
            );
            format!("{fence}\n{}\n{fence}", block.text)
        }
        "quote" => block
            .children
            .iter()
            .map(block_markdown)
            .collect::<Vec<_>>()
            .join("\n\n")
            .split('\n')
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        "heading" => format!("{} {content}", "#".repeat(block.heading_level())),
        "list_item" => format!("- {content}"),
        "table" => table::html(block),
        "list" => crate::renderer::block_html(block),
        _ => content,
    }
}

pub fn render(ir: &Ir) -> String {
    ir.blocks
        .iter()
        .map(block_markdown)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 对可表达的结构生成便于直接编辑的 Markdown。
pub fn render_editable(ir: &Ir) -> String {
    ir.blocks
        .iter()
        .map(editable_block)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn editable_block(block: &Block) -> String {
    if block.kind == "quote" {
        return block
            .children
            .iter()
            .map(editable_block)
            .collect::<Vec<_>>()
            .join("\n\n")
            .split('\n')
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if block.kind == "list" {
        let Some(list) = &block.list else {
            return block_markdown(block);
        };
        let start = list.start.unwrap_or(1);
        let representable = !list.reversed
            && list.custom_style.is_none()
            && (!list.ordered
                || (start > 0 && list.style.as_deref().is_none_or(|style| style == "decimal")))
            && block.children.iter().enumerate().all(|(index, item)| {
                item.kind == "list_item"
                    && item
                        .list_value
                        .is_none_or(|value| value as i128 == start as i128 + index as i128)
            });
        if !representable {
            return block_markdown(block);
        }
        return block
            .children
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let marker = if list.ordered {
                    format!("{}. ", start as i128 + index as i128)
                } else {
                    "- ".into()
                };
                let mut parts = Vec::new();
                if !item.runs.is_empty() {
                    parts.push(item.runs.iter().map(run_markdown).collect::<String>());
                }
                parts.extend(item.children.iter().map(editable_block));
                let body = parts.join("\n\n");
                let indent = " ".repeat(marker.len());
                let mut lines = body.split('\n');
                let mut result = format!("{marker}{}", lines.next().unwrap_or(""));
                for line in lines {
                    result.push_str(&format!("\n{indent}{line}"));
                }
                result
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if block.kind == "table" {
        return table::render(block);
    }
    block_markdown(block)
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_content_and_destinations_and_rejects_unsafe_links() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><p># 标题 [链接] *文本* &lt;script&gt; &amp;copy;</p><p><a \
            href='https://example.org/a(b)?x=1&amp;y=2'>[名称]</a><a href='javascript:alert(1)'>保留文字</a><img \
            src='data:text/html,bad'></p><p><span style='color:red'>[红字]</span></p></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let output = super::render(&ir);
        assert!(output.contains(r"\# 标题 \[链接\] \*文本\* &lt;script&gt; &amp;copy;"));
        assert!(output.contains(r"[\[名称\]](<https://example.org/a(b)?x=1&amp;y=2>)"));
        assert!(output.contains("保留文字"));
        assert!(!output.contains("javascript:"));
        assert!(!output.contains("data:text"));
        assert!(output.contains("<span style=\"color:red\">[红字]</span>"));
    }
}
