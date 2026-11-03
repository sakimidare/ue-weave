//! Check visible text order independently of structural reconstruction.
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
    let evidence = serde_json::json!({"value":if matches {"matched"} else {"mismatch"},"confidence":"high","reason":"对比正文文字、PDF 占位元素的附件标题与重建 HTML 的非空白文字顺序；跳过可执行节点及已知异常字符串，空白和结构另行记录","source_nodes":["n0"],"original_characters":original.chars().count(),"reconstructed_characters":reconstructed.chars().count()});
    ir.metadata
        .insert("content_integrity".into(), evidence.clone());
    if !matches {
        let offset = original
            .chars()
            .zip(reconstructed.chars())
            .take_while(|(a, b)| a == b)
            .count();
        ir.warnings.push(serde_json::json!({"kind":"content-integrity-mismatch","value":evidence,"confidence":"high","reason":"正文文字或顺序存在差异；first_difference 标记首个差异位置，excerpt 字段提供对照片段","source_nodes":["n0"],"first_difference":offset,"original_excerpt":original.chars().skip(offset.saturating_sub(20)).take(80).collect::<String>(),"reconstructed_excerpt":reconstructed.chars().skip(offset.saturating_sub(20)).take(80).collect::<String>()}));
    }
}

#[cfg(test)]
mod tests {
    use scraper::Html;
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
