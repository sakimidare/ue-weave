use super::*;

#[test]
fn explicit_paragraph_spacing_is_retained_without_inference() {
    let document = Html::parse_document("<div class='Article_Content'><p style='margin-top:10px;margin-bottom:0'>正文</p><p>缺省</p></div>");
    let ir = parse(&document, ".Article_Content", true, None).unwrap();
    assert_eq!(ir.blocks[0].spacing["margin-top"].value, "10px");
    assert!(ir.blocks[1].spacing.is_empty());
    let html = crate::renderer::render(&ir);
    assert!(html.contains("margin-top:10px"));
    assert!(html.contains("margin-bottom:0"));
    assert!(ir.warnings.iter().any(|w| w["kind"] == "paragraph-spacing"));
}

#[test]
fn word_vml_images_are_preserved_as_image_references() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p>图前<v:shape style='width:72pt;height:36pt'><v:imagedata \
            src='/word.jpg'></v:imagedata></v:shape>图后</p></div>",
    );
    let ir = parse(
        &document,
        ".Article_Content",
        false,
        Some("https://example.org/article".into()),
    )
    .unwrap();
    assert_eq!(ir.images.len(), 1);
    let attributes = ir.blocks[0].runs[1].image_attributes.as_ref().unwrap();
    assert_eq!(attributes.width.as_deref(), Some("96"));
    assert_eq!(attributes.height.as_deref(), Some("48"));
    assert_eq!(
        ir.blocks[0].runs[1].image.as_deref(),
        Some("https://example.org/word.jpg")
    );
    let html = crate::renderer::render(&ir);
    assert!(html.contains("<img src=\"https://example.org/word.jpg\""));
    assert!(!html.contains("v:shape"));
    assert!(ir
        .warnings
        .iter()
        .any(|warning| warning["kind"] == "vml-image"));
}

#[test]
fn respects_first_base_href_and_rejects_executable_base() {
    let document = Html::parse_document(
        "<head><base href='../assets/'><base href='https://ignored.example/'></head><div \
            class='Article_Content'><p><a href='file.pdf'>附件</a><img src='photo.png'></p></div>",
    );
    let ir = parse(
        &document,
        ".Article_Content",
        false,
        Some("https://example.org/news/page.html".into()),
    )
    .unwrap();
    assert_eq!(
        ir.attachments[0]["href"],
        "https://example.org/assets/file.pdf"
    );
    assert_eq!(
        ir.blocks[0].runs[1].image.as_deref(),
        Some("https://example.org/assets/photo.png")
    );
    let document = Html::parse_document("<base href='javascript:bad'><div class='Article_Content'><p><a href='file.pdf'>附件</a></p></div>");
    let ir = parse(
        &document,
        ".Article_Content",
        false,
        Some("https://example.org/news/page.html".into()),
    )
    .unwrap();
    assert_eq!(
        ir.attachments[0]["href"],
        "https://example.org/news/file.pdf"
    );
    assert!(ir
        .warnings
        .iter()
        .any(|warning| warning["kind"] == "invalid-base-url"));
}

#[test]
fn table_caption_is_editable_and_exported_once() {
    let document = Html::parse_document(include_str!("../fixtures/table-caption.html"));
    let mut ir = parse(&document, ".Article_Content", false, None).unwrap();
    let caption = ir.blocks[0].caption.as_ref().unwrap();
    assert!(caption[0].bold);
    assert_eq!(caption[2].link.as_deref(), Some("https://example.org/info"));
    for output in [
        crate::renderer::render(&ir),
        crate::markdown::render(&ir),
        crate::typst::render(&ir),
    ] {
        assert_eq!(output.matches("课程安排").count(), 1);
    }
    assert!(crate::renderer::render(&ir).contains("<caption><strong>"));
    ir.blocks[0].visit_runs_mut(&mut |run| {
        if run.link.is_some() {
            run.link = Some("local.html".into());
        }
    });
    assert!(crate::renderer::render(&ir).contains("href=\"local.html\""));
}

#[test]
fn inherited_container_styles_survive_block_boundaries() {
    let document = Html::parse_document(
        "<div class='Article_Content' style='color:red;font-weight:bold;text-align:right'><div><p>\
            A<span style='font-weight:normal;color:blue'>B</span>C</p><table><tr><td><p>D</p></td></tr>\
            </table></div></div>",
    );
    let ir = parse(&document, ".Article_Content", true, None).unwrap();
    assert_eq!(ir.blocks[0].align, "right");
    let runs = &ir.blocks[0].runs;
    assert!(runs[0].bold && runs[2].bold);
    assert!(!runs[1].bold);
    assert_eq!(runs[0].color.as_deref(), Some("red"));
    assert_eq!(runs[1].color.as_deref(), Some("blue"));
    let cell = &ir.blocks[1].rows[0][0].blocks[0];
    assert!(cell.runs[0].bold);
    assert_eq!(cell.runs[0].color.as_deref(), Some("red"));
}

#[test]
fn word_residual_styles_map_with_explicit_css_precedence() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p style='horiz-align:right'><span style='mso-foreground:red;\
            mso-highlight:yellow;text-line-through:double'>文字</span><span style='color:blue;\
            mso-foreground:red;color:green'>标准声明</span></p></div>",
    );
    let ir = parse(&document, ".Article_Content", true, None).unwrap();
    assert_eq!(ir.blocks[0].align, "right");
    let run = &ir.blocks[0].runs[0];
    assert_eq!(run.color.as_deref(), Some("red"));
    assert_eq!(run.background_color.as_deref(), Some("yellow"));
    assert!(run.strike);
    assert_eq!(ir.blocks[0].runs[1].color.as_deref(), Some("green"));
    assert!(ir
        .warnings
        .iter()
        .any(|warning| warning["kind"] == "word-style-mapping"));
    let html = crate::renderer::render(&ir);
    assert!(!html.contains("mso-"));
    assert!(html.contains("<s>文字</s>"));
}

#[test]
fn quote_code_and_rule_preserve_structure_and_whitespace() {
    let document = Html::parse_document(include_str!("../fixtures/quote-code.html"));
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(
        ir.blocks
            .iter()
            .map(|b| b.kind.as_str())
            .collect::<Vec<_>>(),
        ["quote", "horizontal_rule", "code_block"]
    );
    assert!(ir.blocks[2].text.starts_with("  let value"));
    assert!(ir.blocks[2].text.contains("\n    <tag>"));
    let html = crate::renderer::render(&ir);
    assert_eq!(html.matches("<blockquote>").count(), 2);
    assert!(html.contains("<hr>"));
    assert!(html.contains("&lt;tag&gt;"));
    let markdown = crate::markdown::render(&ir);
    assert!(markdown.contains("> > 内层引用"));
    assert!(markdown.contains("````\n  let value"));
    assert!(crate::typst::render(&ir).contains("#raw("));
}

#[test]
fn nested_lists_preserve_groups_and_numbering() {
    let document = Html::parse_document(include_str!("../fixtures/nested-lists.html"));
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks.len(), 3);
    assert_eq!(ir.blocks[0].list.as_ref().unwrap().start, Some(3));
    assert_eq!(ir.blocks[0].children[0].children[1].kind, "list");
    let output = crate::renderer::render(&ir);
    assert!(output.starts_with("<ol start=\"3\"><li>"));
    assert_eq!(output.matches("内层甲").count(), 1);
    assert_eq!(output.matches("<ul>").count(), 2);
    assert!(crate::typst::render(&ir).contains("#enum(start: 3,"));
}

#[test]
fn mixed_direct_text_and_blocks_keep_order() {
    let document = Html::parse_document("<div class='Article_Content'>开头<span>行内</span><p>段落</p>中间<table><tr><td>格前<p>格段</p>格后<img \
        src='/a.png'></td></tr></table>结尾</div>");
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let output = crate::renderer::render(&ir);
    let parsed = Html::parse_fragment(&output);
    let text: String = parsed.root_element().text().collect();
    assert_eq!(text.replace('\n', ""), "开头行内段落中间格前格段格后结尾");
    assert_eq!(output.matches("<img").count(), 1);
    assert_eq!(ir.blocks[3].rows[0][0].blocks.len(), 3);
}

#[test]
fn nested_tables_render_each_paragraph_once() {
    let document = Html::parse_document(
        "<div class='Article_Content'><table><tr><td><p>首段</p><table><tr><td><p>内段</p></td></tr></table>\
            <p>尾段</p></td></tr></table></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks.len(), 1);
    assert_eq!(ir.blocks[0].rows[0][0].blocks.len(), 3);
    for output in [
        crate::renderer::render(&ir),
        crate::markdown::render(&ir),
        crate::typst::render(&ir),
    ] {
        for text in ["首段", "内段", "尾段"] {
            assert_eq!(output.matches(text).count(), 1);
        }
    }
}

#[test]
fn preserves_linked_image_attributes() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p><a href='/large'><img src='/photo.jpg' alt='图 [一] &quot;\
            说明&quot;' title='标题' width='640' height='151'></a></p></div>",
    );
    let ir = parse(
        &document,
        ".Article_Content",
        false,
        Some("https://example.org/page".into()),
    )
    .unwrap();
    let run = &ir.blocks[0].runs[0];
    assert_eq!(
        run.image_attributes.as_ref().unwrap().width.as_deref(),
        Some("640")
    );
    let html = crate::renderer::render(&ir);
    assert!(html.contains("<a href=\"https://example.org/large\"><img"));
    assert!(html.contains("alt=\"图 [一] &quot;说明&quot;\""));
    assert!(html.contains("width=\"640\" height=\"151\""));
    let md = crate::markdown::render(&ir);
    assert!(md.contains(r"[![图 \[一\]"));
    assert!(md.contains("https://example.org/large"));
}

#[test]
fn paragraph_indent_and_line_height_survive_roundtrip() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p style='text-indent:2em;line-height:1.5em'>正文</p><p \
            style='text-indent:0em;line-height:normal'>取消缩进</p><p>缺省</p></div>",
    );
    let ir = parse(&document, ".Article_Content", true, None).unwrap();
    assert_eq!(ir.blocks[0].line_height.as_ref().unwrap().value, "1.5em");
    assert!(ir.blocks[2].line_height.is_none());
    let rendered = crate::renderer::render(&ir);
    assert!(rendered.contains("text-indent:2em;line-height:1.5em"));
    assert!(rendered.contains("text-indent:0em;line-height:normal"));
    assert!(!rendered.contains("data-inferred"));
    let restored = parse(
        &Html::parse_document(&format!("<div class='Article_Content'>{rendered}</div>")),
        ".Article_Content",
        true,
        None,
    )
    .unwrap();
    assert_eq!(restored.blocks[0].indent.value, ir.blocks[0].indent.value);
    assert_eq!(
        restored.blocks[1].line_height.as_ref().unwrap().value,
        "normal"
    );
}

#[test]
fn table_cells_remain_editable_and_rendered() {
    let doc = Html::parse_document(
        "<div class='Article_Content'><table><tr><th colspan='2'>Header</th></tr><tr><td rowspan='2'><b>\
            A</b></td><td>B</td></tr><tr><td>C</td></tr></table></div>",
    );
    let ir = parse(&doc, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].rows.len(), 3);
    assert_eq!(ir.blocks[0].rows[0][0].colspan, 2);
    assert_eq!(ir.blocks[0].rows[1][0].rowspan, 2);
    let html = crate::renderer::render(&ir);
    assert!(html.contains("<strong>A</strong>"));
    assert!(html.contains("<th colspan=\"2\""));
    assert!(crate::markdown::render(&ir).contains("<table>"));
}

#[test]
fn native_headings_survive_disabled_inference() {
    let doc = Html::parse_document("<div class='Article_Content'><h3>Title</h3><p>  <b>五、安排</b></p><p style='text-align:right'>教务处</p></div>");
    let ir = parse(&doc, ".Article_Content", true, None).unwrap();
    assert_eq!(ir.blocks[0].heading_level(), 3);
    assert_eq!(ir.blocks[1].kind, "paragraph");
    assert_ne!(ir.blocks[1].indent.value, "likely");
    assert!(ir.blocks[2].signature.is_none());
    assert!(crate::renderer::render(&ir).starts_with("<h3>Title</h3>"));
    assert!(crate::markdown::render(&ir).starts_with("### Title"));
    assert!(crate::typst::render(&ir).starts_with("=== "));
    assert_eq!(
        parse(&doc, ".Article_Content", false, None).unwrap().blocks[1].kind,
        "heading"
    );
}

#[test]
fn explicit_breaks_survive_all_exports() {
    let doc = Html::parse_document("<div class='Article_Content'><p>A<br><br>B\nC</p></div>");
    let ir = parse(&doc, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].runs.iter().filter(|r| r.line_break).count(), 2);
    assert_eq!(crate::renderer::render(&ir), "<p>A<br><br>B\nC</p>");
    assert!(crate::markdown::render(&ir).contains("A<br>\n<br>\nB"));
    assert!(
        crate::typst::render(&ir).contains("#text(\"A\")#linebreak()#linebreak()#text(\"B\\nC\")")
    );
    let restored: crate::model::Ir =
        serde_json::from_str(&serde_json::to_string(&ir).unwrap()).unwrap();
    assert_eq!(
        crate::renderer::render(&restored),
        crate::renderer::render(&ir)
    );
}

#[test]
fn ueditor_font_and_basestyle_rules() {
    let doc = Html::parse_document(concat!(
        r#"<div class="Article_Content"><p><font size="3">A</font><sup>B</sup><sub>C</sub>"#,
        r#"<span style="text-decoration:underline line-through">D</span><b>"#,
        r#"E<span style="font-weight:normal;font-style:italic">F</span>G</b></p></div>"#,
    ));
    let ir = parse(&doc, ".Article_Content", false, None).unwrap();
    let runs = &ir.blocks[0].runs;
    assert_eq!(runs[0].font_size.as_deref(), Some("16px"));
    assert_eq!(runs[1].vertical_align.as_deref(), Some("super"));
    assert_eq!(runs[2].vertical_align.as_deref(), Some("sub"));
    assert!(runs[3].underline && runs[3].strike);
    assert!(runs[4].bold && runs[6].bold);
    assert!(!runs[5].bold && runs[5].italic);
    let html = crate::renderer::render(&ir);
    assert!(html.contains("<sup>B</sup>") && html.contains("<sub>C</sub>"));
    assert!(html.contains("<u><s>D</s></u>"));
}

const FIXTURE: &str = concat!(
    r#"<div class="Article_Content"><p>&nbsp;&nbsp;<strong>一、面向对象</strong></p><p>正文 <em>"#,
    r#"斜体</em> undefinedundefined</p><p><a href="/a.doc">"#,
    r#"<img class="sudy-attachment-icon" src="icon_doc.gif">附件</a></p><p>"#,
    r#"<img src="image.png"></p><ul><li>列表</li></ul><p style="text-align:right">教务处</p>"#,
    r#"</div>"#,
);

#[test]
fn parses_editor_specific_structure() {
    let document = Html::parse_document(FIXTURE);
    let ir = parse(&document, ".Article_Content", false, Some("fixture".into())).unwrap();
    assert!(ir.blocks.iter().any(|block| block.kind == "heading"));
    assert_eq!(ir.attachments[0]["href"], "/a.doc");
    assert_eq!(ir.images[0]["src"], "image.png");
    assert!(ir
        .warnings
        .iter()
        .any(|warning| warning["kind"] == "undefinedundefined"));
    assert!(ir.blocks.iter().any(|block| block
        .signature
        .as_ref()
        .map(|x| x.value == serde_json::json!(true))
        .unwrap_or(false)));
}

#[test]
fn resolves_relative_links_against_source_url() {
    let document = Html::parse_document(
        r#"<div class="Article_Content"><p><a href="/files/a.pdf">附件</a><img src="images/a.png"></p></div>"#,
    );
    let ir = parse(
        &document,
        ".Article_Content",
        false,
        Some("https://jwc.seu.cn/2026/notice/page.htm".into()),
    )
    .unwrap();
    assert_eq!(
        ir.blocks[0].runs[0].link.as_deref(),
        Some("https://jwc.seu.cn/files/a.pdf")
    );
    assert_eq!(
        ir.blocks[0].runs[1].image.as_deref(),
        Some("https://jwc.seu.cn/2026/notice/images/a.png")
    );
}

#[test]
fn preserves_common_ueditor_inline_styles() {
    let document = Html::parse_document(concat!(
        r#"<div class="Article_Content"><p>"#,
        r#"<span style="font-family:宋体;font-size:16px;color:#f00;"#,
        r#"background-color:#ff0;vertical-align:super"><b>x</b></span></p></div>"#,
    ));
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let run = &ir.blocks[0].runs[0];
    assert_eq!(run.font_family.as_deref(), Some("宋体"));
    assert_eq!(run.font_size.as_deref(), Some("16px"));
    assert_eq!(run.color.as_deref(), Some("#f00"));
    assert_eq!(run.background_color.as_deref(), Some("#ff0"));
    assert_eq!(run.vertical_align.as_deref(), Some("super"));
}
#[test]
fn unknown_wrappers_preserve_nested_paragraph_boundaries() {
    let document = Html::parse_document("<div class='Article_Content'>前<custom-box style='font-weight:bold'>甲<p>乙</p><custom-inner><p>\
        丙</p></custom-inner>丁</custom-box><figure><figcaption>图注</figcaption></figure>后</div>");
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let texts: Vec<String> = ir
        .blocks
        .iter()
        .map(|block| block.runs.iter().map(|run| run.text.as_str()).collect())
        .collect();
    assert_eq!(texts, ["前", "甲", "乙", "丙", "丁", "图注", "后"]);
    for block in &ir.blocks[1..5] {
        assert!(block.runs.iter().all(|run| run.bold));
    }
    assert!(!ir.blocks[6].runs[0].bold);
}
#[test]
fn list_item_values_reset_numbering_in_all_exports() {
    let document = Html::parse_document(include_str!("../fixtures/list-values.html"));
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].children[1].list_value, Some(8));
    let html = crate::renderer::render(&ir);
    assert!(html.contains("<li value=\"8\">"));
    assert!(html.contains("<li value=\"-1\">"));
    assert!(crate::markdown::render(&ir).contains("<li value=\"8\">"));
    let typst = crate::typst::render(&ir);
    for number in [3, 8, 9, 4] {
        assert!(typst.contains(&format!("enum.item({number})")));
    }
    for number in [-1, 0, 1] {
        assert!(typst.contains(&format!("#text(\"{number}.\")")));
    }
    let restored: crate::model::Ir =
        serde_json::from_str(&serde_json::to_string(&ir).unwrap()).unwrap();
    assert_eq!(crate::renderer::render(&restored), html);
}
#[test]
fn css_comments_and_priority_match_diagnostics() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p style='/* note; */ text-align:right !/*x*/important;\
            text-align:left;/* end */'>正文</p></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].align, "right");
    assert!(crate::renderer::render(&ir).contains("text-align:right"));
    assert!(ir.warnings.iter().any(|w| w["kind"] == "css-priority"));
    assert!(ir
        .warnings
        .iter()
        .any(|w| w["kind"] == "css-comment-normalization"));
    assert!(!ir.warnings.iter().any(|w| matches!(
        w["kind"].as_str(),
        Some("malformed-style" | "unsafe-style" | "unmapped-style")
    )));
}
#[test]
fn shorthand_paragraph_spacing_survives_html_roundtrip() {
    let document = Html::parse_document("<div class='Article_Content'><p style='margin:2px 0 6px!important;margin-bottom:9px'>正文</p></div>");
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].spacing["margin-top"].value, "2px");
    assert_eq!(ir.blocks[0].spacing["margin-bottom"].value, "6px");
    let html = crate::renderer::render(&ir);
    assert!(html.contains("margin-top:2px"));
    assert!(html.contains("margin-bottom:6px"));
}
#[test]
fn inherited_line_height_has_resolvable_provenance() {
    let document = Html::parse_document(
        "<body style='line-height:9'><div class='Article_Content' style='line-height:1.5'><p>甲</p><p \
            style='line-height:initial'>乙</p><div style='line-height:2em'><p style='line-height:inherit'>\
            丙</p></div></div></body>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    for (index, value) in ["1.5", "normal", "2em"].iter().enumerate() {
        let evidence = ir.blocks[index].line_height.as_ref().unwrap();
        assert_eq!(evidence.value, *value);
        for id in &evidence.source_nodes {
            assert!(ir.original_nodes.iter().any(|node| node["id"] == *id));
        }
    }
    assert!(ir
        .warnings
        .iter()
        .any(|w| w["kind"] == "relative-line-height"));
}
#[test]
fn inherited_font_keywords_and_resets_are_scoped() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p style='font-family:Example;font-size:20px;color:red;\
            font-weight:bold;font-style:italic'>甲<span style='font-family:inherit;font-size:unset;\
            color:INHERIT'>乙</span><b style='font-weight:initial;font-style:initial;font-family:initial;\
            font-size:initial;color:initial'>丙</b>丁</p></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let runs = &ir.blocks[0].runs;
    assert_eq!(runs.len(), 4);
    for index in [0, 1, 3] {
        assert_eq!(runs[index].font_family.as_deref(), Some("Example"));
        assert_eq!(runs[index].font_size.as_deref(), Some("20px"));
        assert_eq!(runs[index].color.as_deref(), Some("red"));
        assert!(runs[index].bold && runs[index].italic);
    }
    assert!(
        runs[2].font_family.is_none() && runs[2].font_size.is_none() && runs[2].color.is_none()
    );
    assert!(!runs[2].bold && !runs[2].italic);
    assert!(!crate::typst::render(&ir).contains("\"inherit\""));
}
#[test]
fn legacy_list_types_preserve_case_and_css_precedence() {
    let document = Html::parse_document(
        "<div class='Article_Content'><ol type='A'><li>甲</li></ol><ol type='i'><li>乙</li></ol><ol \
            type='a' style='list-style-type:upper-roman'><li>丙</li></ol><ul type='SQUARE'><li>丁</li></ul></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    for (block, expected) in
        ir.blocks
            .iter()
            .zip(["upper-alpha", "lower-roman", "upper-roman", "square"])
    {
        assert_eq!(
            block.list.as_ref().unwrap().style.as_deref(),
            Some(expected)
        );
    }
    let html = crate::renderer::render(&ir);
    assert!(html.contains("list-style-type:square"));
    let typst = crate::typst::render(&ir);
    assert!(typst.contains("marker: [#text(\"▪\")]"));
    for pattern in ["A.", "i.", "I."] {
        assert!(typst.contains(&format!("numbering: \"{pattern}\"")));
    }
}
#[test]
fn ueditor_custom_list_type_keeps_evidence_and_reports_visual_limits() {
    let document = Html::parse_document(
        "<div class='Article_Content'><ol class='custom_cn list-paddingleft-1'><li class='list-cn-1-1'>\
            甲</li><li>乙</li></ol></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let evidence = ir.blocks[0]
        .list
        .as_ref()
        .unwrap()
        .custom_style
        .as_ref()
        .unwrap();
    assert_eq!(evidence.value, "cn");
    assert_eq!(evidence.confidence, "high");
    assert!(ir
        .warnings
        .iter()
        .any(|warning| warning["kind"] == "custom-list-marker"));
    let html = crate::renderer::render(&ir);
    assert!(html.contains("甲") && html.contains("乙"));
    assert!(!html.contains("custom_cn"));
}
#[test]
fn cleaned_block_text_matches_runs_and_code_has_diagnostics() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p>甲undefinedundefined<br>乙<script>忽略</script></p><pre>\
        代码undefinedundefined\n第二行</pre></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].text, "甲\n乙");
    assert_eq!(ir.blocks[1].text, "代码undefinedundefined\n第二行");
    assert_eq!(
        ir.warnings
            .iter()
            .filter(|w| w["kind"] == "undefinedundefined")
            .count(),
        1
    );
    for output in [
        crate::renderer::render(&ir),
        crate::markdown::render(&ir),
        crate::typst::render(&ir),
    ] {
        assert!(output.contains("代码undefinedundefined"));
        assert!(!output.contains("忽略"));
    }
    assert!(serde_json::to_string(&ir.original_nodes)
        .unwrap()
        .contains("undefinedundefined"));
}
#[test]
fn table_row_groups_preserve_distinct_bodies_and_source_nodes() {
    let document = Html::parse_document(
        "<div class='Article_Content'><table><thead><tr><th>表头</th></tr></thead><tbody><tr><td \
            rowspan='0'>甲</td></tr></tbody><tbody><tr><td>乙</td></tr></tbody><tfoot><tr><td>合计</td></tr>\
            </tfoot></table></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let groups = &ir.blocks[0].row_groups;
    assert_eq!(groups.len(), 4);
    assert!(groups[1] != groups[2]);
    for group in groups.iter().flatten() {
        assert!(ir
            .original_nodes
            .iter()
            .any(|node| node["id"] == group.source_node && node["tag"] == group.tag));
    }
    let html = crate::renderer::render(&ir);
    assert_eq!(html.matches("<tbody>").count(), 2);
    assert!(html.contains("<thead>"));
    assert!(html.contains("<tfoot>"));
    assert_eq!(ir.blocks[0].rows[1][0].rowspan, 1);
}
#[test]
fn word_spacing_and_dimensions_map_without_discarding_units() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p style='mso-margin-alt:3pt 0 6pt;margin-top:2px'>正文</p><table>\
            <tr><td style='mso-padding-alt:2pt 4pt;mso-width:5cm;mso-height:20pt'>单元格</td></tr></table></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    assert_eq!(ir.blocks[0].spacing["margin-top"].value, "2px");
    assert_eq!(ir.blocks[0].spacing["margin-bottom"].value, "6pt");
    let cell = &ir.blocks[1].rows[0][0];
    assert_eq!(cell.styles["padding-left"].value, "4pt");
    assert_eq!(cell.styles["width"].value, "5cm");
    let html = crate::renderer::render(&ir);
    assert!(html.contains("padding-left:4pt"));
    assert!(html.contains("height:20pt"));
    assert!(!html.contains("mso-"));
    assert!(ir
        .warnings
        .iter()
        .any(|w| w["kind"] == "word-style-mapping" && w["attribute"] == "mso-margin-alt"));
}
#[test]
fn image_css_dimensions_and_link_survive_exports() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p><a href='https://example.org'><img src='images/test.png' \
            width='800' style='width:50%;height:120px' alt='图示'></a></p></div>",
    );
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    let html = crate::renderer::render(&ir);
    assert!(html.contains("width:50%;height:120px"));
    assert!(!html.contains("width=\"800\""));
    let typst = crate::typst::render(&ir);
    assert!(typst.contains("#link(\"https://example.org\")[#image("));
    assert!(typst.contains("width: 50%, height: 90pt, alt: \"图示\""));
}
#[test]
fn word_paragraph_structure_is_evidenced_without_removing_markers() {
    let document = Html::parse_document(
        "<div class='Article_Content'><p class='MsoListParagraph'>1. 甲</p><p style='mso-list:l0 level2 \
            lfo1'>a) 乙</p><p class='MsoHeading'>标题</p><p>1. 普通段落</p></div>",
    );
    for disabled in [false, true] {
        let ir = parse(&document, ".Article_Content", disabled, None).unwrap();
        assert_eq!(ir.blocks[0].text, "1. 甲");
        assert_eq!(ir.blocks[1].text, "a) 乙");
        assert_eq!(
            ir.blocks[0].word_structure.as_ref().unwrap().value,
            if disabled {
                "unknown"
            } else {
                "likely-list-paragraph"
            }
        );
        assert_eq!(
            ir.blocks[2].word_structure.as_ref().unwrap().value,
            if disabled {
                "unknown"
            } else {
                "likely-heading"
            }
        );
        assert!(ir.blocks[3].word_structure.is_none());
    }
}
#[test]
fn extension_attributes_and_empty_wrappers_have_complete_treatments() {
    let document = Html::parse_document("<div class='Article_Content'><span></span><div></div><p><a href='file.doc' sudyfile-attr='{}' \
        isupload='true'><img class='sudy-attachment-icon' src='icon_doc.gif'>附件</a></p></div>");
    let ir = parse(&document, ".Article_Content", false, None).unwrap();
    for kind in ["empty-element", "ue-or-word-attribute", "attachment-icon"] {
        let records: Vec<_> = ir.warnings.iter().filter(|w| w["kind"] == kind).collect();
        assert!(!records.is_empty());
        for record in records {
            for field in [
                "original",
                "removed",
                "ir_field",
                "inferred",
                "confidence",
                "reason",
                "source_nodes",
            ] {
                assert!(record.get(field).is_some(), "{kind}: {field}");
            }
        }
    }
    assert!(crate::renderer::render(&ir).contains("附件</a>"));
}
