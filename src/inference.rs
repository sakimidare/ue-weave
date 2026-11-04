use crate::diagnostic::{Details, Diagnostic, NoteKind};
use crate::model::{Block, Ir};
use serde_json::json;

pub fn annotate(ir: &mut Ir, disabled: bool) {
    let word_nodes: Vec<_> = ir
        .original_nodes
        .iter()
        .filter(|node| {
            let tag = node["tag"].as_str().unwrap_or("");
            let class = node["attrs"]["class"].as_str().unwrap_or("");
            let style = node["attrs"]["style"].as_str().unwrap_or("");
            tag.starts_with("o:")
                || tag.starts_with("v:")
                || tag.starts_with("w:")
                || class.split_whitespace().any(|c| c.starts_with("Mso"))
                || style.to_ascii_lowercase().contains("mso-")
        })
        .map(|node| node["id"].clone())
        .collect();
    let likely = !disabled && !word_nodes.is_empty();
    let evidence = json!({
        "value": if likely {"likely"} else {"unknown"},
        "confidence": if likely {"medium"} else {"unknown"},
        "reason": if disabled {"推断已关闭"} else if likely {"检测到 Mso 类、mso 样式或 Office 命名空间，据此推断 Word 来源"} else {"未检测到 Word 来源标记，来源记为 unknown"},
        "source_nodes": word_nodes,
    });
    ir.metadata.insert("word_origin".into(), evidence.clone());
    if !word_nodes.is_empty() {
        ir.warnings.push(Diagnostic::new(
            NoteKind::WordOrigin,
            Details {
                evidence: serde_json::json!(evidence),
                source_nodes: word_nodes
                    .iter()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .collect(),
                ..Default::default()
            },
        ));
    }
    for block in &ir.blocks {
        empty_paragraphs(block, disabled, &mut ir.warnings);
    }
    if !disabled {
        signatures(ir);
    }
}

fn leaves<'a>(blocks: &'a mut [Block], output: &mut Vec<&'a mut Block>) {
    for block in blocks {
        if block.children.is_empty() && block.rows.is_empty() {
            output.push(block);
        } else {
            leaves(&mut block.children, output);
            for cell in block.rows.iter_mut().flatten() {
                leaves(&mut cell.blocks, output);
            }
        }
    }
}

fn signatures(ir: &mut Ir) {
    let mut ordered = Vec::new();
    leaves(&mut ir.blocks, &mut ordered);
    for block in ordered
        .into_iter()
        .rev()
        .filter(|b| !b.text.trim().is_empty())
        .take(3)
    {
        let text = block.text.trim();
        let unit = ["处", "学院", "大学", "办公室", "中心", "委员会", "部门"]
            .iter()
            .any(|suffix| text.ends_with(suffix));
        let date = text.chars().filter(char::is_ascii_digit).count() >= 6
            && (text.contains('年') || text.contains('-') || text.contains('/'));
        if block.kind != "paragraph"
            || block.align != "right"
            || text.chars().count() > 30
            || !(unit || date)
        {
            break;
        }
        let evidence = crate::model::Evidence {
            value: json!(true),
            confidence: "medium".into(),
            reason: "正文末尾连续右对齐短段落，含单位名称或日期特征".into(),
            source_nodes: block.source_nodes.clone(),
        };
        ir.warnings.push(Diagnostic::new(
            NoteKind::SignatureInference,
            Details {
                text: Some((text).to_owned()),
                align: Some((block.align).to_owned()),
                value: serde_json::json!("signature"),
                confidence: Some((evidence.confidence).to_owned()),
                reason: Some((evidence.reason).to_owned()),
                source_nodes: evidence.source_nodes.clone(),
                ..Default::default()
            },
        ));
        block.signature = Some(evidence);
        block.kind = "signature".into();
    }
}

fn empty_paragraphs(block: &Block, disabled: bool, warnings: &mut Vec<Diagnostic>) {
    if block.kind == "paragraph"
        && block.children.is_empty()
        && block.runs.iter().all(|r| {
            r.image.is_none()
                && r.text
                    .chars()
                    .all(|c| c.is_whitespace() || matches!(c, '\u{200b}' | '\u{feff}'))
        })
    {
        let likely = !disabled && block.runs.iter().any(|r| r.line_break);
        warnings.push(Diagnostic::new(
            NoteKind::EmptyParagraph,
            Details {
                original: serde_json::json!(block.raw_html),
                removed: Some(false),
                ir_field: Some("blocks".to_owned()),
                inferred: Some(likely),
                value: serde_json::json!(if likely {
                    "likely-editor-placeholder"
                } else {
                    "unknown"
                }),
                confidence: Some((if likely { "low" } else { "unknown" }).to_owned()),
                reason: Some(
                    (if likely {
                        "空段落含 br，按编辑器占位结构作低置信度推断"
                    } else {
                        "段落无可见文字，记录为空段落；生成原因记为 unknown"
                    })
                    .to_owned(),
                ),
                source_nodes: block.source_nodes.clone(),
                ..Default::default()
            },
        ));
    }
    for child in &block.children {
        empty_paragraphs(child, disabled, warnings);
    }
    for cell in block.rows.iter().flatten() {
        for child in &cell.blocks {
            empty_paragraphs(child, disabled, warnings);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn signatures_require_trailing_position_and_semantic_evidence() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><p style='text-align:right'>教务处</p><p>后续正文</p><p \
            style='text-align:right'>教务处</p><p style='text-align:right'>2026年10月25日</p><p><br></p></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        assert!(ir.blocks[0].signature.is_none());
        assert_eq!(ir.blocks[2].kind, "signature");
        assert_eq!(ir.blocks[3].kind, "signature");
        assert!(ir.blocks[2]
            .signature
            .as_ref()
            .unwrap()
            .reason
            .contains("末尾"));
    }
    #[test]
    fn inference_is_evidenced_and_can_be_disabled() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><p class='MsoNormal'><br></p><p><img src='x.png'></p><p> </p></div>");
        for disabled in [false, true] {
            let ir = crate::parser::parse(&document, ".Article_Content", disabled, None).unwrap();
            assert_eq!(
                ir.metadata["word_origin"]["value"],
                if disabled { "unknown" } else { "likely" }
            );
            let empty: Vec<_> = ir
                .warnings
                .iter()
                .filter(|w| w["kind"] == "empty-paragraph")
                .collect();
            assert_eq!(empty.len(), 2);
            assert_eq!(empty[0]["inferred"], !disabled);
            assert_eq!(ir.blocks.len(), 3);
        }
    }
}
