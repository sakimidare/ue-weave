use comfy_table::{
    ColumnConstraint, ContentArrangement, ContentLineStyle, Table, TableStyle, Width,
};

pub(super) struct Pane {
    pub text: String,
    pub anchor: usize,
}

pub(super) fn width() -> usize {
    terminal_size::terminal_size_of(std::io::stderr())
        .map(|(width, _)| usize::from(width.0))
        .unwrap_or(120)
}

pub(super) fn pair(left: Pane, right: Pane, width: usize) -> String {
    if width < 100 {
        return format!("HTML 上下文\n{}\n清洗后文本\n{}\n", left.text, right.text);
    }
    let anchor = left.anchor.max(right.anchor);
    let left_lines: Vec<_> = std::iter::repeat_n("", anchor - left.anchor)
        .chain(left.text.lines())
        .collect();
    let right_lines: Vec<_> = std::iter::repeat_n("", anchor - right.anchor)
        .chain(right.text.lines())
        .collect();
    let mut table = Table::new();
    table
        .load_style(TableStyle::new().content_lines(ContentLineStyle::none().junction('│')))
        .set_width(width.min(u16::MAX as usize) as u16)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth);
    for i in 0..left_lines.len().max(right_lines.len()) {
        table.add_row([
            left_lines.get(i).copied().unwrap_or(""),
            right_lines.get(i).copied().unwrap_or(""),
        ]);
    }
    table.set_constraints([ColumnConstraint::Absolute(Width::Fixed(((width - 1) / 2) as u16)); 2]);
    format!("{table}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_diagnostics_keep_separator_at_the_same_display_column() {
        for width in [100, 120, 151, 200] {
            let mut expected = None;
            for (left, right) in [
                ("短".to_string(), "另一侧很长".repeat(30)),
                ("带颜色中文😀".repeat(30), "短".into()),
                ("\u{1b}[31m中文😀\u{1b}[0m\n第二行".into(), "正文".into()),
            ] {
                let rendered = pair(
                    Pane {
                        text: left,
                        anchor: 0,
                    },
                    Pane {
                        text: right,
                        anchor: 0,
                    },
                    width,
                );
                for line in rendered.lines() {
                    let (left, _) = line.split_once('│').unwrap();
                    let column = console::measure_text_width(left);
                    assert_eq!(
                        *expected.get_or_insert(column),
                        column,
                        "width={width}: {line}"
                    );
                    assert!(console::measure_text_width(line) <= width);
                }
            }
        }
    }
    #[test]
    fn chinese_and_colored_annotations_share_a_row() {
        let result = pair(
            Pane {
                text: "<div>\n<p>中文😀</p>\n\u{1b}[31m^^ HTML\u{1b}[0m".into(),
                anchor: 2,
            },
            Pane {
                text: "正文\n^^ TEXT".into(),
                anchor: 1,
            },
            120,
        );
        let row = result
            .lines()
            .find(|line| line.contains("^^ HTML"))
            .unwrap();
        assert!(row.contains("^^ TEXT"));
        let stacked = pair(
            Pane {
                text: "左".into(),
                anchor: 0,
            },
            Pane {
                text: "右".into(),
                anchor: 0,
            },
            80,
        );
        assert_eq!(stacked, "HTML 上下文\n左\n清洗后文本\n右\n");
    }
}
