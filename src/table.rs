use scraper::ElementRef;

use crate::style_registry::{CELL_STYLES, TABLE_STYLES};

pub fn table_styles(
    table: ElementRef,
    source: &str,
) -> std::collections::BTreeMap<String, crate::model::Evidence> {
    TABLE_STYLES
        .iter()
        .filter_map(|key| {
            let value = crate::rules::style(table, key).or_else(|| {
                if !matches!(*key, "width" | "height") {
                    return None;
                }
                table.value().attr(key).map(|value| {
                    if value.parse::<u32>().is_ok() {
                        format!("{value}px")
                    } else {
                        value.into()
                    }
                })
            })?;
            Some((
                key.to_string(),
                crate::rules::evidence(
                    serde_json::json!(value),
                    "high",
                    "最终 HTML 中显式表格样式或尺寸属性",
                    source,
                ),
            ))
        })
        .collect()
}

pub fn column_groups(
    table: ElementRef,
    snapshot: &crate::snapshot::Snapshot,
) -> Vec<crate::model::TableColumn> {
    fn column(
        node: ElementRef,
        snapshot: &crate::snapshot::Snapshot,
        group: bool,
    ) -> crate::model::TableColumn {
        let source = snapshot.element_id(node);
        let width = crate::rules::style(node, "width").or_else(|| {
            node.value().attr("width").map(|value| {
                if value.parse::<u32>().is_ok() {
                    format!("{value}px")
                } else {
                    value.into()
                }
            })
        });
        crate::model::TableColumn {
            span: node
                .value()
                .attr("span")
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 1000),
            width: width.map(|width| {
                crate::rules::evidence(
                    serde_json::json!(width),
                    "high",
                    "原始列定义中的显式宽度声明",
                    source,
                )
            }),
            children: if group {
                node.children()
                    .filter_map(ElementRef::wrap)
                    .filter(|child| child.value().name() == "col")
                    .map(|child| column(child, snapshot, false))
                    .collect()
            } else {
                Vec::new()
            },
            source_nodes: vec![source.into()],
        }
    }
    table
        .children()
        .filter_map(ElementRef::wrap)
        .filter(|node| node.value().name() == "colgroup")
        .map(|node| column(node, snapshot, true))
        .collect()
}

pub fn columns_html(groups: &[crate::model::TableColumn]) -> String {
    fn attrs(column: &crate::model::TableColumn) -> String {
        let mut attrs = format!(" span=\"{}\"", column.span.clamp(1, 1000));
        if let Some(width) = column
            .width
            .as_ref()
            .and_then(|e| e.value.as_str())
            .filter(|v| crate::safety::safe_css(v))
        {
            attrs.push_str(&format!(
                " style=\"width:{}\"",
                html_escape::encode_double_quoted_attribute(width)
            ));
        }
        attrs
    }
    groups
        .iter()
        .map(|group| {
            let children = group
                .children
                .iter()
                .map(|column| format!("<col{}>", attrs(column)))
                .collect::<String>();
            format!("<colgroup{}>{children}</colgroup>", attrs(group))
        })
        .collect()
}

pub fn cell_styles(
    cell: ElementRef,
    source: &str,
) -> std::collections::BTreeMap<String, crate::model::Evidence> {
    let mut styles = std::collections::BTreeMap::new();
    for property in CELL_STYLES {
        let value = crate::rules::style(cell, property).or_else(|| {
            let attr = match *property {
                "text-align" => "align",
                "vertical-align" => "valign",
                "background-color" => "bgcolor",
                "width" => "width",
                "height" => "height",
                _ => return None,
            };
            cell.value().attr(attr).map(|value| {
                if matches!(*property, "width" | "height") && value.parse::<u32>().is_ok() {
                    format!("{value}px")
                } else {
                    value.into()
                }
            })
        });
        if let Some(value) = value {
            styles.insert(
                property.to_string(),
                crate::rules::evidence(
                    serde_json::json!(value),
                    "high",
                    "原始单元格 CSS 或 HTML 属性声明",
                    source,
                ),
            );
        }
    }
    styles
}

/// Resolve spans against the remaining rows in this thead/tbody/tfoot group.
pub fn row_span(cell: ElementRef, row: ElementRef) -> usize {
    let remaining = 1 + row
        .next_siblings()
        .filter_map(ElementRef::wrap)
        .filter(|node| node.value().name() == "tr")
        .count();
    match cell
        .value()
        .attr("rowspan")
        .and_then(|value| value.trim().parse::<usize>().ok())
    {
        Some(0) => remaining,
        Some(value) => value.min(remaining).min(65534),
        None => 1,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn explicit_table_styles_survive_and_unsafe_values_are_filtered() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><table width='800' style='width:90%;border-collapse:collapse;border-spacing:3px;table-layout:fixed;background-color:url(bad)'><tr><td>正文</td></tr></table></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        assert_eq!(ir.blocks[0].table_styles["width"].value, "90%");
        let output = crate::renderer::render(&ir);
        for declaration in [
            "width:90%",
            "border-collapse:collapse",
            "border-spacing:3px",
            "table-layout:fixed",
        ] {
            assert!(output.contains(declaration));
        }
        assert!(!output.contains("url(bad)"));
        assert!(ir
            .warnings
            .iter()
            .any(|w| w["kind"] == "table-style" && w["removed"] == true));
    }
    #[test]
    fn typst_preserves_four_side_and_zero_padding() {
        let document = scraper::Html::parse_document(include_str!("../fixtures/cell-padding.html"));
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let output = crate::typst::render(&ir);
        assert!(output.contains("inset: (top: 2pt, right: 4pt, bottom: 6pt, left: 8pt)"));
        assert!(output.contains("inset: 0pt"));
        assert!(output.contains("inset: (top: 1.5pt, right: 1.5pt, bottom: 1.5pt, left: 7.5pt)"));
    }
    #[test]
    fn typst_column_widths_expand_spans_and_group_defaults() {
        let document =
            scraper::Html::parse_document(include_str!("../fixtures/column-widths.html"));
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        assert!(crate::typst::render(&ir).contains("columns: (45pt, 45pt, 25%,)"));
    }
    #[test]
    fn column_definitions_keep_width_span_and_nested_table_ownership() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><table><colgroup width='80'><col span='2' style='width:25%'><col width='120'></colgroup><tbody><tr><td><table><colgroup span='3' width='40'><tbody><tr><td>内层</td></tr></tbody></table></td></tr></tbody></table></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let groups = &ir.blocks[0].column_groups;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].width.as_ref().unwrap().value, "80px");
        assert_eq!(groups[0].children.len(), 2);
        assert_eq!(groups[0].children[0].span, 2);
        assert_eq!(groups[0].children[0].width.as_ref().unwrap().value, "25%");
        let html = crate::renderer::render(&ir);
        assert!(html.contains("<col span=\"2\" style=\"width:25%\">"));
        assert!(html.contains("<colgroup span=\"3\" style=\"width:40px\">"));
        assert_eq!(html.matches("内层").count(), 1);
    }
    #[test]
    fn typst_maps_alignment_fill_and_padding() {
        let document = scraper::Html::parse_document(include_str!("../fixtures/table-styles.html"));
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let typst = crate::typst::render(&ir);
        assert!(typst.contains("align: right + horizon"));
        assert!(typst.contains("fill: rgb(\"#ffeeaa\")"));
        assert!(typst.contains("inset: 6pt"));
        assert!(typst.contains("align: center + bottom"));
    }
    #[test]
    fn cell_styles_preserve_declarations_and_filter_unsafe_output() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><table><tr><td align='right' width='120' style='border:1px solid red;background-color:yellow'>A</td><td style='background-color:url(bad)'>B</td></tr></table></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        let cell = &ir.blocks[0].rows[0][0];
        assert_eq!(cell.styles["width"].value, "120px");
        assert_eq!(cell.styles["text-align"].value, "right");
        assert!(!cell.styles["border"].source_nodes.is_empty());
        let html = crate::renderer::render(&ir);
        assert!(html.contains("border:1px solid red"));
        assert!(html.contains("background-color:yellow"));
        assert!(!html.contains("url(bad)"));
    }
    #[test]
    fn zero_span_stops_at_row_group_boundary() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><table><tbody><tr><td rowspan='0'>A</td><td>B</td></tr><tr><td>C</td></tr></tbody><tbody><tr><td rowspan='9'>D</td><td>E</td></tr></tbody></table></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        assert_eq!(ir.blocks[0].rows[0][0].rowspan, 2);
        assert_eq!(ir.blocks[0].rows[2][0].rowspan, 1);
        let html = crate::renderer::render(&ir);
        assert!(html.contains("rowspan=\"2\">A"));
        assert!(!html.contains("rowspan=\"0\""));
        assert!(ir
            .warnings
            .iter()
            .any(|warning| warning["kind"] == "rowspan-normalized"));
    }
}
