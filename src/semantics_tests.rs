use crate::{model::Ir, parser};

fn parse(source: &str) -> Ir {
    parser::parse(
        &crate::html::parse(source),
        ".Article_Content",
        true,
        Some("https://example.edu/article".into()),
    )
    .unwrap()
}

#[test]
fn code_preserves_literal_content_and_language_in_all_exports() {
    let ir = parse(include_str!("../fixtures/ueditor-semantics.html"));
    let block = &ir.blocks[0];
    assert_eq!(block.code_language.as_deref(), Some("python"));
    assert_eq!(
        block.text,
        "if True:\n    print(\"undefinedundefined\")\n    print(\"<tag>\")"
    );
    assert_eq!(ir.blocks[1].code_language.as_deref(), Some("rust"));
    assert!(ir.blocks[1].text.contains("{\n    println!"));
    assert!(ir.blocks[2].text.contains('\u{a0}'));
    assert!(crate::renderer::render(&ir).contains("class=\"language-python\""));
    assert!(crate::markdown::render(&ir).contains("```python\nif True:"));
    assert!(crate::typst::render(&ir).contains("lang: \"python\""));
    let json = crate::document::from_analysis(&ir);
    let restored = crate::ir_format::read(
        &serde_json::to_vec(&json).unwrap(),
        std::path::Path::new("edited.json"),
    )
    .unwrap();
    assert_eq!(restored.blocks[0].text, block.text);
    assert_eq!(restored.blocks[0].code_language, block.code_language);
    assert_eq!(
        crate::renderer::render(&restored),
        crate::renderer::render(&ir)
    );
    let mut invalid = json;
    invalid["blocks"][0]["code_language"] = "python\n```".into();
    assert!(crate::ir_format::read(
        &serde_json::to_vec(&invalid).unwrap(),
        std::path::Path::new("edited.json")
    )
    .is_err());
}

#[test]
fn structures_media_and_static_alternatives_keep_order_and_addresses() {
    let ir = parse(include_str!("../fixtures/ueditor-semantics.html"));
    let html = crate::renderer::render(&ir);
    assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
    assert_eq!(ir.metadata["resource_integrity"]["value"], "matched");
    assert!(html.contains("<dl><dt>"));
    assert!(html.contains("</dd><dt>"));
    assert!(html.contains("text-align:center"));
    assert!(html.contains("margin-left:auto;margin-right:auto"));
    assert!(html.contains("<code>a`b &lt; c</code>"));
    assert!(html.contains("<kbd>Ctrl+C</kbd>"));
    assert!(!html.contains("临时书签"));
    assert!(!html.contains("svg-noise"));
    for path in [
        "video.mp4",
        "video.webm",
        "audio.mp3",
        "player",
        "fallback.png",
    ] {
        assert!(
            html.contains(&format!("https://example.edu/{path}")),
            "{path}"
        );
    }
    assert!(html.contains("统计图 年度数据 增长百分之十"));
    assert!(html.contains("x+y"));
    for tag in [
        "section", "article", "col", "colgroup", "dl", "dt", "dd", "code", "kbd", "samp", "var",
    ] {
        assert!(
            !ir.warnings
                .iter()
                .any(|w| w["kind"] == "unsupported-structure" && w["original"]["tag"] == tag),
            "{tag}"
        );
    }
    assert!(crate::markdown::render(&ir).contains("``a`b < c``"));
    assert!(crate::typst::render(&ir).contains("#raw(\"Ctrl+C\")"));
}

#[test]
fn deep_html_and_nested_blocks_are_exportable() {
    for (open, close, count) in [
        ("<div>", "</div>", 5000),
        ("<blockquote>", "</blockquote>", 80),
        ("<table><tr><td>", "</td></tr></table>", 60),
    ] {
        let source = format!(
            "<div class='Article_Content'>{}保留文字{}</div>",
            open.repeat(count),
            close.repeat(count)
        );
        let ir = parse(&source);
        assert!(ir.warnings.iter().any(|w| w["code"] == "W0019"));
        for output in [
            crate::renderer::render(&ir),
            crate::markdown::render(&ir),
            crate::typst::render(&ir),
        ] {
            assert!(output.contains("保留文字"));
        }
        let json = serde_json::to_vec(&crate::document::from_analysis(&ir)).unwrap();
        crate::ir_format::read(&json, std::path::Path::new("deep.json")).unwrap();
    }
}

#[test]
fn code_survives_profiles_and_inline_ir_roundtrip() {
    let ir = parse(concat!(
        "<div class='Article_Content'><pre class='brush:python'>\n&nbsp;&nbsp;x = 'undefinedundefined'\n</pre>",
        "<p><code>  undefinedundefined  </code></p></div>"
    ));
    for profile in [
        crate::profile::Profile::Conservative,
        crate::profile::Profile::Balanced,
        crate::profile::Profile::Aggressive,
    ] {
        let output = crate::profile::prepare(&ir, profile.settings());
        assert_eq!(output.ir.blocks[0].text, ir.blocks[0].text);
        assert_eq!(output.ir.blocks[1].runs[0].text, "  undefinedundefined  ");
        assert_eq!(
            output.ir.blocks[1].runs[0]
                .inline_kind
                .map(|kind| kind.tag()),
            Some("code")
        );
    }
    assert!(!ir
        .warnings
        .iter()
        .any(|w| w["kind"] == "undefinedundefined"));
    assert!(ir.warnings.iter().any(|w| w["code"] == "N0032"));
    assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
}

#[test]
fn table_placement_does_not_center_cell_text() {
    let ir = parse("<div class='Article_Content'><table align='center'><tr><td><p>单元格</p></td></tr></table></div>");
    assert_eq!(ir.blocks[0].rows[0][0].blocks[0].align, "unknown");
    assert!(crate::typst::render(&ir).contains("#align(center)"));
}

#[test]
fn bookmark_and_foreign_content_do_not_add_downloads() {
    let ir = parse(concat!(
        "<div class='Article_Content'><p>正文<span id='_baidu_bookmark_1'><img src='/hidden.png'>",
        "<a href='/hidden.pdf'>书签</a></span></p><svg><text>图中文字</text><a href='/svg.pdf'>svg链接</a></svg></div>"
    ));
    assert!(ir.images.is_empty());
    assert!(ir.attachments.is_empty());
    assert!(ir.warnings.iter().any(|w| w["code"] == "N0031"));
    assert_eq!(ir.metadata["resource_integrity"]["value"], "matched");
}

#[test]
fn adjacent_inline_code_uses_unambiguous_markup() {
    let ir = parse("<div class='Article_Content'><p><code>a<span>b</span>c</code></p></div>");
    assert_eq!(
        crate::markdown::render(&ir),
        "<code>a</code><code>b</code><code>c</code>"
    );
    assert_eq!(ir.blocks[0].text, "abc");
}

#[test]
fn partial_containers_report_the_lost_container_semantics() {
    let ir = parse(concat!(
        "<div class='Article_Content'><section><p>正文</p></section>",
        "<figure><img src='/diagram.png'><figcaption>图注</figcaption></figure>",
        "<details><summary>摘要</summary><p>详情</p></details></div>"
    ));
    for tag in ["figure", "figcaption", "details", "summary"] {
        assert!(ir
            .warnings
            .iter()
            .any(|w| w["kind"] == "unsupported-structure" && w["original"]["tag"] == tag));
    }
    assert!(!ir
        .warnings
        .iter()
        .any(|w| w["kind"] == "unsupported-structure" && w["original"]["tag"] == "section"));
    assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
    assert_eq!(ir.metadata["resource_integrity"]["value"], "matched");
}

#[test]
fn media_urls_are_checked_and_fallback_text_is_kept() {
    let ir = parse(concat!(
        "<div class='Article_Content'><iframe src='javascript:alert(1)'></iframe>",
        "<video src='javascript:alert(2)'><source src='/ok.webm'>视频说明</video></div>"
    ));
    let html = crate::renderer::render(&ir);
    assert!(!html.contains("javascript:"));
    assert!(html.contains("https://example.edu/ok.webm"));
    assert!(html.contains("视频说明"));
    assert_eq!(ir.metadata["resource_integrity"]["value"], "matched");
}
