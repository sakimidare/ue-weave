//! Deterministic boundary corpus, independent of network availability.
#[test]
fn mixed_containers_keep_text_and_resource_order() {
    for body in [
        "前<div>甲<p>乙</p>丙<div>丁</div>戊</div>后",
        "<figure>前<img src='a.png' alt='说明'><figcaption>图注</figcaption>后</figure>",
        "<dl><dt>名称</dt><dd>解释<p>补充</p>尾</dd></dl>",
        "<ul><li>前<p>段落</p>后<ul><li>子项</li></ul></li></ul>",
        "<table><tr><td>前<p>中</p>后</td></tr></table>",
        "<V:SHAPE xmlns:v='urn:schemas-microsoft-com:vml' style='width:20pt'><V:IMAGEDATA src='a.png' o:title='图片'></V:IMAGEDATA></V:SHAPE>",
        "<p>前<o:p xmlns:o='urn:schemas-microsoft-com:office:office'>中</o:p>后</p>",
        "<p><![CDATA[内容]]>后<p>下一段",
    ] {
        let doc = scraper::Html::parse_document(&format!("<div class='Article_Content'>{body}</div>"));
        let ir = crate::parser::parse(&doc, ".Article_Content", true, None).unwrap();
        assert_eq!(ir.metadata["content_integrity"]["value"], "matched", "{body}");
        assert_eq!(ir.metadata["resource_integrity"]["value"], "matched", "{body}");
    }
}

#[test]
fn generated_inputs_finish_without_panics() {
    let tokens = [
        "<p>",
        "</div>",
        "<span style='",
        "&nbsp;",
        "\"",
        "</p>",
        "<![CDATA[",
        "中",
        "<v:shape>",
        "<!--",
        "url(",
        "\\",
        "'",
        ">",
        "\0",
    ];
    let mut seed = 0x12345678u64;
    for _ in 0..256 {
        let mut value = String::new();
        for _ in 0..32 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            value.push_str(tokens[(seed >> 32) as usize % tokens.len()]);
        }
        let _ = crate::attachment::parse_metadata(&value);
        let _ = crate::css::property(&value, "font-family");
        let _ = crate::safety::safe_css(&value);
        let _ = crate::safety::safe_url(&value, true);
        let _ = crate::ir_format::read(value.as_bytes(), std::path::Path::new("generated.json"));
        let document =
            scraper::Html::parse_document(&format!("<div class='Article_Content'>{value}</div>"));
        if let Ok(ir) = crate::parser::parse(&document, ".Article_Content", true, None) {
            let output =
                crate::profile::prepare(&ir, crate::profile::Profile::Aggressive.settings());
            let _ = crate::export::render(
                &output.ir,
                crate::profile::Profile::Aggressive.settings(),
                &Default::default(),
            );
        }
    }
}
