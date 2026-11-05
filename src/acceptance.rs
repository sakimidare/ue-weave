//! 使用已保存的公开文章正文进行离线验收。
use scraper::{Html, Selector};

fn visible_text(html: &str) -> String {
    let document = Html::parse_fragment(html);
    let mut text = String::new();
    for node in document.root_element().descendants() {
        if let scraper::Node::Text(value) = node.value() {
            text.push_str(value);
        }
        if let Some(element) = scraper::ElementRef::wrap(node) {
            if element.value().name() == "span"
                && element.value().attr("pdfsrc").is_some()
                && element.text().collect::<String>().trim().is_empty()
            {
                let attributes: serde_json::Value = serde_json::from_str(
                    &element
                        .value()
                        .attr("sudyfile-attr")
                        .unwrap()
                        .replace('\'', "\""),
                )
                .unwrap();
                text.push_str(attributes["title"].as_str().unwrap());
            }
        }
    }
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
        let prepared = crate::profile::prepare_documents(
            &ir,
            crate::profile::Profile::Conservative.settings(),
        );
        let rendered = crate::renderer::render(&prepared.ir);
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
        let restored = crate::profile::prepare_documents(
            &restored,
            crate::profile::Profile::Conservative.settings(),
        );
        assert_eq!(crate::renderer::render(&restored.ir), rendered);
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

#[test]
fn college_snapshots_preserve_body_assets_and_metadata() {
    for (source, url, title, date) in [
        (
            include_str!("../fixtures/xsxy-534301.html"),
            "https://xsxy.seu.edu.cn/2025/0624/c57168a534301/page.htm",
            "六朝松",
            "2025-06-24",
        ),
        (
            include_str!("../fixtures/cse-588376.html"),
            "https://cse.seu.edu.cn/2026/1102/c22535a588376/page.htm",
            "学院开展“企业家进高校”前沿大课堂系列活动（二）",
            "2026-11-02",
        ),
        (
            include_str!("../fixtures/cse-585944.html"),
            "https://cse.seu.edu.cn/2026/1014/c49342a585944/page.htm",
            "【研究生】2026级博士新生奖学金评奖工作的通知",
            "2026-10-14",
        ),
        (
            include_str!("../fixtures/ee-586046.html"),
            "https://ee.seu.edu.cn/2026/1015/c25183a586046/page.htm",
            "2026年全球前2%顶尖科学奖榜单出炉！东南大学电气工程学院65人次登榜！",
            "2026-10-14",
        ),
        (
            include_str!("../fixtures/ee-581788.html"),
            "https://ee.seu.edu.cn/2026/0907/c25184a581788/page.htm",
            "电气工程学院导师招收2027级研究生资格审核工作细则",
            "2026-09-07",
        ),
    ] {
        let doc = Html::parse_document(source);
        let selector = crate::site::content_selector(&doc, "auto").unwrap();
        assert_eq!(selector, ".wp_articlecontent");
        let root = doc
            .select(&Selector::parse(&selector).unwrap())
            .next()
            .unwrap();
        let ir = crate::parser::parse(&doc, &selector, false, Some(url.into())).unwrap();
        assert_eq!(ir.metadata["title"], title, "{url}");
        assert_eq!(ir.metadata["date"], date, "{url}");
        assert!(ir.metadata.get("publisher").is_some());
        assert!(ir.metadata.get("author").is_none());
        for profile in [
            crate::profile::Profile::Conservative,
            crate::profile::Profile::Balanced,
            crate::profile::Profile::Aggressive,
        ] {
            let output = crate::profile::prepare(&ir, profile.settings());
            let rendered = crate::renderer::render(&output.ir);
            if url.contains("581788") {
                assert!(rendered.contains("电气工程学院导师招收2027级研究生资格审核工作细则.pdf"));
                assert!(rendered.contains(".pdf"));
            } else {
                assert_eq!(visible_text(&root.html()), visible_text(&rendered), "{url}");
            }
            assert_eq!(
                ir.metadata["content_integrity"]["value"], "matched",
                "{url}"
            );
            assert!(!rendered.contains("sudy-attachment-icon"));
            assert!(!rendered.contains("icon_doc.gif"));
            assert!(
                !crate::markdown::render(&output.ir).is_empty(),
                "{url} {profile:?}: {}",
                serde_json::to_string(&output.ir.blocks).unwrap()
            );
            assert!(!crate::typst::render(&output.ir).is_empty());
        }
        if title == "六朝松" {
            assert!(root.text().collect::<String>().trim().is_empty());
            assert_eq!(ir.images.len(), 1);
            assert_eq!(ir.blocks.len(), 1);
        }
        if url.contains("585944") {
            assert_eq!(ir.attachments.len(), 4);
            assert!(ir.attachments.iter().all(|a| a["href"]
                .as_str()
                .unwrap()
                .starts_with("https://cse.seu.edu.cn/")));
        }
        if url.contains("581788") {
            assert_eq!(ir.attachments.len(), 1);
        }
    }
}

#[test]
fn real_merged_table_and_missing_metadata() {
    let source = include_str!("../fixtures/seu-578974.html");
    let doc = Html::parse_document(source);
    let selector = crate::site::content_selector(&doc, "auto").unwrap();
    let ir = crate::parser::parse(
        &doc,
        &selector,
        false,
        Some("https://jwc.seu.edu.cn/2026/0810/c21676a578974/page.htm".into()),
    )
    .unwrap();
    assert!(!ir.metadata.contains_key("title"));
    assert!(!ir.metadata.contains_key("date"));
    assert!(!ir.attachments.is_empty());
    let rendered = crate::renderer::render(&ir);
    assert_eq!(visible_text(source), visible_text(&rendered));
    let output = Html::parse_fragment(&rendered);
    let spans = |doc: &Html| {
        doc.select(&Selector::parse("[rowspan]").unwrap())
            .filter_map(|node| {
                node.value()
                    .attr("rowspan")
                    .and_then(|s| s.parse::<usize>().ok())
            })
            .filter(|&n| n > 1)
            .collect::<Vec<_>>()
    };
    assert_eq!(spans(&doc), spans(&output));
    assert!(spans(&output).contains(&8));
}

#[test]
fn absent_college_metadata_does_not_pick_navigation_titles() {
    let mut doc = Html::parse_document(include_str!("../fixtures/ee-586046.html"));
    let ids: Vec<_> = doc
        .select(&Selector::parse(".arti_title, .arti_metas").unwrap())
        .map(|node| node.id())
        .collect();
    for id in ids {
        doc.tree.get_mut(id).unwrap().detach();
    }
    let doc = Html::parse_document(&doc.root_element().html());
    assert!(doc
        .select(&Selector::parse(".Article_Title").unwrap())
        .next()
        .is_some());
    let ir = crate::parser::parse(&doc, ".wp_articlecontent", false, None).unwrap();
    for field in ["title", "date", "author", "publisher"] {
        assert!(!ir.metadata.contains_key(field));
    }
    assert!(!ir.blocks.is_empty());
}
