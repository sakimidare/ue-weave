//! Network-independent checks against captured public article bodies.
use scraper::{Html, Selector};

fn visible_text(html: &str) -> String {
    let document = Html::parse_fragment(html);
    let text: String = document.root_element().text().collect();
    crate::rules::clean_sentinel(&text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

#[test]
fn real_articles_preserve_text_order_and_asset_counts() {
    for (source, url, attachments, images) in [
        (
            include_str!("../fixtures/seu-585911.html"),
            "https://jwc.seu.edu.cn/2026/1013/c71422a585911/page.htm",
            2,
            0,
        ),
        (
            include_str!("../fixtures/seu-583204.html"),
            "https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm",
            1,
            0,
        ),
        (
            include_str!("../fixtures/seu-583286.html"),
            "https://jwc.seu.edu.cn/2026/0916/c21678a583286/page.htm",
            2,
            1,
        ),
    ] {
        let document = Html::parse_document(source);
        let ir =
            crate::parser::parse(&document, ".Article_Content", false, Some(url.into())).unwrap();
        let rendered = crate::renderer::render(&ir);
        assert_eq!(
            ir.metadata["content_integrity"]["value"], "matched",
            "{url}"
        );
        assert_eq!(
            visible_text(source),
            visible_text(&rendered),
            "正文遗漏、重复或顺序变化：{url}"
        );
        assert_eq!(ir.attachments.len(), attachments, "{url}");
        assert_eq!(ir.images.len(), images, "{url}");
        assert!(!rendered.contains("undefinedundefined"));
        assert!(!rendered.contains("sudy-attachment-icon"));
        assert!(!rendered.contains("icon_doc.gif"));
        let output = Html::parse_fragment(&rendered);
        for link in output.select(&Selector::parse("a").unwrap()) {
            assert!(
                link.value().attr("href").unwrap().starts_with("https://"),
                "{url}"
            );
        }
        let restored: crate::model::Ir =
            serde_json::from_str(&serde_json::to_string(&ir).unwrap()).unwrap();
        assert_eq!(crate::renderer::render(&restored), rendered);
    }
}

#[test]
fn sample_has_numbered_headings_and_trailing_signatures() {
    let document = Html::parse_document(include_str!("../fixtures/seu-585911.html"));
    let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks.iter().filter(|b| b.kind == "heading").count(), 4);
    assert_eq!(
        ir.blocks.iter().filter(|b| b.kind == "signature").count(),
        2
    );
    assert!(ir.warnings.iter().any(|w| w["kind"] == "whitespace"));
}
