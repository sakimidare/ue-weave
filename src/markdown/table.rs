//! 生成 GFM 表格，合并单元格和嵌套结构使用 HTML 表达。
use super::{block_markdown, run_markdown};
use crate::model::{Block, TableCell};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Header {
    Original,
    Empty,
}

pub(crate) fn header(block: &Block) -> Option<Header> {
    let first = block.rows.first()?;
    if first.is_empty()
        || !block.rows.iter().all(|row| {
            row.len() == first.len()
                && row.iter().all(|cell| {
                    cell.colspan == 1
                        && cell.rowspan == 1
                        && cell.blocks.iter().all(|b| {
                            b.kind == "paragraph" && b.children.is_empty() && b.rows.is_empty()
                        })
                })
        })
    {
        return None;
    }
    if block.rows.iter().flatten().all(|cell| !cell.header) {
        Some(Header::Empty)
    } else if first.iter().all(|cell| cell.header)
        && block.rows.iter().skip(1).flatten().all(|cell| !cell.header)
    {
        Some(Header::Original)
    } else {
        None
    }
}

fn cell(cell: &TableCell) -> String {
    let mut parts = Vec::new();
    if !cell.runs.is_empty() {
        parts.push(cell.runs.iter().map(run_markdown).collect::<String>());
    }
    parts.extend(cell.blocks.iter().map(block_markdown));
    parts
        .join("<br><br>")
        .replace("\\|", "&#124;")
        .replace('|', "&#124;")
        .replace("<br>\n", "<br>")
        .replace("\r\n", "\n")
        .replace(['\n', '\r'], "<br>")
}

fn row(cells: impl IntoIterator<Item = String>) -> String {
    format!("| {} |", cells.into_iter().collect::<Vec<_>>().join(" | "))
}

pub(super) fn render(block: &Block) -> String {
    let Some(header) = header(block) else {
        return html(block);
    };
    let columns = block.rows[0].len();
    let separator = row(vec!["---".into(); columns]);
    let mut lines = Vec::new();
    if let Some(caption) = &block.caption {
        lines.push(caption.iter().map(run_markdown).collect());
        lines.push(String::new());
    }
    if header == Header::Empty {
        lines.push(row(vec![String::new(); columns]));
        lines.push(separator.clone());
    }
    for (index, cells) in block.rows.iter().enumerate() {
        lines.push(row(cells.iter().map(cell)));
        if index == 0 && header == Header::Original {
            lines.push(separator.clone());
        }
    }
    lines.join("\n")
}

pub(super) fn html(block: &Block) -> String {
    // 仅在结构标签之间插入排版空白，保留行内文字的间距。
    crate::renderer::block_html(block)
        .replace("<tr", "\n<tr")
        .replace("</tr>", "</tr>\n")
        .replace("\n\n", "\n")
}

#[cfg(test)]
mod tests {
    fn export(body: &str) -> String {
        let document =
            scraper::Html::parse_document(&format!("<div class='Article_Content'>{body}</div>"));
        let ir = crate::parser::parse(&document, ".Article_Content", true, None).unwrap();
        super::render(&ir.blocks[0])
    }

    #[test]
    fn headerless_table_preserves_first_row_and_inline_content() {
        let output = export("<table><caption>说明</caption><tr><td></td><td><b>机器人</b></td></tr><tr><td><p>报名</p><p>条件</p>\
            </td><td>A|B<br><a href='https://example.org'>链接</a><img src='a.png'></td></tr></table>");
        assert!(output.starts_with("说明\n\n|  |  |\n| --- | --- |\n|  | **机器人** |"));
        assert!(output.contains("报名<br><br>条件"));
        assert!(
            output.contains("A&#124;B<br>[链接](<https://example.org/>)")
                || output.contains("A&#124;B<br>[链接](<https://example.org>)")
        );
        assert!(!output.contains("A&#124;B<br><br>"));
        assert!(output.contains("![](<a.png>)"));
        assert_eq!(
            output.lines().filter(|line| line.starts_with('|')).count(),
            4
        );
    }

    #[test]
    fn explicit_header_and_complex_tables_keep_their_structure() {
        assert_eq!(
            export(
                "<table><tr><th>名称</th><th>数量</th></tr><tr><td>A</td><td>2</td></tr></table>"
            ),
            "| 名称 | 数量 |\n| --- | --- |\n| A | 2 |"
        );
        for body in [
            "<table><tr><td rowspan='2'>A</td><td>B</td></tr><tr><td>C</td></tr></table>",
            "<table><tr><th>A</th><td>B</td></tr></table>",
            "<table><tr><td><table><tr><td>嵌套</td></tr></table></td></tr></table>",
            "<table><tr><td><pre>  A\n  B</pre></td></tr></table>",
        ] {
            let output = export(body);
            assert!(output.starts_with("<table>"), "{output}");
            assert!(output.contains("\n<tr>"));
            let before = scraper::Html::parse_fragment(body);
            let after = scraper::Html::parse_fragment(&output);
            for tag in ["table", "tr", "td", "th", "pre"] {
                let selector = scraper::Selector::parse(tag).unwrap();
                assert_eq!(
                    before.select(&selector).count(),
                    after.select(&selector).count()
                );
            }
        }
    }
}
