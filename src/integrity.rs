//! 单独校验可见文字内容及顺序。
use crate::diagnostic::{Details, Diagnostic, WarningKind};
use scraper::{ElementRef, Html, Node};

fn visible_text(root: ElementRef) -> String {
    let mut text = String::new();
    for node in root.descendants() {
        if node.ancestors().filter_map(ElementRef::wrap).any(|parent| {
            matches!(
                parent.value().name(),
                "script" | "style" | "iframe" | "object" | "embed"
            )
        }) {
            continue;
        }
        if let Some(element) = ElementRef::wrap(node) {
            if let Some((_, title)) = crate::attachment::embedded_pdf(element) {
                text.push_str(&title);
            }
        }
        if let Node::Text(value) = node.value() {
            let excluded = node
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|parent| crate::attachment::embedded_pdf(parent).is_some());
            if !excluded {
                text.push_str(value);
            }
        }
    }
    crate::rules::clean_sentinel(&text)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

pub fn check(root: ElementRef, ir: &mut crate::model::Ir) {
    let original = visible_text(root);
    let output = Html::parse_fragment(&crate::renderer::render(ir));
    let reconstructed = visible_text(output.root_element());
    let matches = original == reconstructed;
    let evidence = serde_json::json!({
        "value": if matches {"matched"} else {"mismatch"},
        "confidence": "high",
        "reason": "对比正文文字、PDF 占位元素的附件标题与重建 HTML 的非空白文字顺序；跳过可执行节点及已知异常字符串，空白和结构另行记录",
        "source_nodes": ["n0"],
        "original_characters": original.chars().count(),
        "reconstructed_characters": reconstructed.chars().count(),
    });
    ir.metadata
        .insert("content_integrity".into(), evidence.clone());
    if !matches {
        let offset = original
            .chars()
            .zip(reconstructed.chars())
            .take_while(|(a, b)| a == b)
            .count();
        ir.warnings.push(Diagnostic::new(
            WarningKind::ContentIntegrityMismatch,
            Details {
                value: serde_json::json!(evidence),
                confidence: Some("high".to_owned()),
                reason: Some("正文文字或顺序存在差异".to_owned()),
                source_nodes: vec![("n0").to_string()],
                first_difference: Some(offset),
                original_excerpt: Some(
                    (original
                        .chars()
                        .skip(offset.saturating_sub(20))
                        .take(80)
                        .collect::<String>())
                    .to_owned(),
                ),
                reconstructed_excerpt: Some(
                    (reconstructed
                        .chars()
                        .skip(offset.saturating_sub(20))
                        .take(80)
                        .collect::<String>())
                    .to_owned(),
                ),
                ..Default::default()
            },
        ));
    }
    let base = ir
        .metadata
        .get("resolved_base_url")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| url::Url::parse(value).ok());
    let expected = resources(root, &base);
    let actual = resources(output.root_element(), &base);
    let resources_match = expected == actual;
    ir.metadata.insert(
        "resource_integrity".into(),
        serde_json::json!({
            "value": if resources_match {"matched"} else {"mismatch"},
            "expected": expected,
            "actual": actual,
            "reason": "独立统计正文图片、超链接和 PDF 阅读器地址，比较 URL 与出现次数",
        }),
    );
    if !resources_match {
        ir.warnings.push(Diagnostic::new(
            WarningKind::ResourceIntegrityMismatch,
            Details {
                source_nodes: vec![("n0").to_string()],
                expected: serde_json::json!(expected),
                actual: serde_json::json!(actual),
                reason: Some("资源地址或数量发生变化，详见 expected 与 actual".to_owned()),
                ..Default::default()
            },
        ));
    }
}

fn resources(
    root: ElementRef,
    base: &Option<url::Url>,
) -> std::collections::BTreeMap<String, usize> {
    let mut result = std::collections::BTreeMap::new();
    for node in root.descendants().filter_map(ElementRef::wrap) {
        if node.ancestors().filter_map(ElementRef::wrap).any(|parent| {
            matches!(
                parent.value().name(),
                "script" | "style" | "iframe" | "object" | "embed"
            ) || (parent
                .value()
                .classes()
                .any(|class| class == "wp_pdf_player")
                && parent.value().attr("pdfsrc").is_some())
        }) {
            continue;
        }
        let pdf = node.value().classes().any(|class| class == "wp_pdf_player");
        let resource = if pdf {
            node.value().attr("pdfsrc").map(|url| ("link", url))
        } else {
            match node.value().name() {
                "img" | "v:imagedata" if !crate::rules::attachment_icon(node) => {
                    node.value().attr("src").map(|url| ("image", url))
                }
                "a" => node.value().attr("href").map(|url| ("link", url)),
                _ => None,
            }
        };
        if let Some((kind, url)) = resource {
            if crate::safety::safe_url(url, kind == "image") {
                let url = crate::rules::absolute_url(base.as_ref(), url);
                *result.entry(format!("{kind}:{url}")).or_insert(0) += 1;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use scraper::Html;
    #[test]
    fn detects_image_loss_even_when_visible_text_matches() {
        let doc =
            Html::parse_document("<div class='Article_Content'><p>正文<img src='a.png'></p></div>");
        let root = doc
            .select(&scraper::Selector::parse(".Article_Content").unwrap())
            .next()
            .unwrap();
        let mut ir = crate::parser::parse(&doc, ".Article_Content", false, None).unwrap();
        assert_eq!(ir.metadata["resource_integrity"]["value"], "matched");
        ir.blocks[0].runs.retain(|run| run.image.is_none());
        super::check(root, &mut ir);
        assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
        assert_eq!(ir.metadata["resource_integrity"]["value"], "mismatch");
    }
    #[test]
    fn detects_loss_duplication_and_order_changes() {
        let document = Html::parse_document(
            "<div class='Article_Content'><p>甲乙丙</p><script>跳过</script></div>",
        );
        let root = document
            .select(&scraper::Selector::parse(".Article_Content").unwrap())
            .next()
            .unwrap();
        for edited in ["甲丙", "甲乙乙丙", "丙乙甲"] {
            let mut ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
            assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
            ir.blocks[0].runs[0].text = edited.into();
            super::check(root, &mut ir);
            assert_eq!(ir.metadata["content_integrity"]["value"], "mismatch");
            assert!(ir
                .warnings
                .iter()
                .any(|warning| warning["kind"] == "content-integrity-mismatch"));
        }
    }
}
