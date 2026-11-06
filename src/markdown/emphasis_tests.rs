use super::*;

fn parse(source: &str) -> Ir {
    let document = crate::html::parse(source);
    crate::parser::parse(&document, "body", true, None).unwrap()
}

fn markdown_html(markdown: &str) -> String {
    let mut html = String::new();
    pulldown_cmark::html::push_html(
        &mut html,
        pulldown_cmark::Parser::new_ext(
            markdown,
            pulldown_cmark::Options::ENABLE_TABLES | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
        ),
    );
    html
}

#[test]
fn definition_terms_and_font_names_survive_all_profiles() {
    use crate::profile::{prepare_documents, Profile};
    let ir = parse(
        r#"<p><span style='font-family:"ACME,  Inc"'>字体</span></p><dl><dt><strong>术语</strong></dt><dd>解释</dd><dt>混合<strong>加粗</strong><em>斜体</em></dt><dd>说明</dd></dl>"#,
    );
    for profile in [
        Profile::Conservative,
        Profile::Balanced,
        Profile::Aggressive,
    ] {
        let settings = profile.settings();
        let prepared = prepare_documents(&ir, settings);
        let markdown = if settings.native_markdown {
            render_editable(&prepared.ir)
        } else {
            render(&prepared.ir)
        };
        assert!(!markdown.contains("****"), "{markdown}");
        let html = markdown_html(&markdown);
        assert!(html.contains("<strong>术语</strong>"), "{html}");
        assert!(html.contains("解释"));
        assert!(html.contains("<em>斜体</em>"));
        if matches!(profile, Profile::Conservative) {
            assert!(crate::renderer::render(&prepared.ir).contains("ACME,  Inc"));
        }
    }
}

#[test]
fn cse_dates_years_and_table_cells_use_single_emphasis_pairs() {
    let ir = parse(include_str!("../../fixtures/cse-emphasis.html"));
    let original = serde_json::to_string(&ir).unwrap();
    let prepared =
        crate::profile::prepare_documents(&ir, crate::profile::Profile::Aggressive.settings());
    let markdown = render_editable(&prepared.ir);
    assert!(
        markdown.contains("**2022（五年制）、2023、2024、2025级**"),
        "{markdown}"
    );
    assert!(markdown.contains("**10月16日—11月6日**"), "{markdown}");
    assert!(
        markdown.contains("| **2023级** | **“至善学子”奖** |"),
        "{markdown}"
    );
    assert!(!markdown.contains("****"));
    let html = markdown_html(&markdown);
    assert!(html.contains("<strong>10月16日—11月6日</strong>"));
    assert!(html.contains("<strong>“至善学子”奖</strong>"));
    assert!(!crate::renderer::render(&prepared.ir).contains("</strong><strong>"));
    assert_eq!(serde_json::to_string(&ir).unwrap(), original);
    let conservative =
        crate::profile::prepare_documents(&ir, crate::profile::Profile::Conservative.settings());
    let html = crate::renderer::render(&conservative.ir);
    assert!(html.contains("font-family:Arial"));
    assert!(html.contains("font-family:Calibri"));
}

#[test]
fn emphasis_boundaries_keep_text_and_format_after_markdown_parsing() {
    for source in [
        "<p><b>甲</b><b><i>乙</i></b><i>丙</i></p>",
        "<p><b>甲</b><b><a href='https://example.org/'>乙</a></b><b>丙</b></p>",
        "<p><b> 甲 </b><i>乙</i><s> 丙 </s></p>",
        "<p><b>甲</b><br><b>乙</b></p>",
        "<p><code>甲</code><code>乙</code></p>",
        "<p>甲<b>“乙”</b>丙</p>",
        "<p>甲<i>（乙）</i>丙</p>",
    ] {
        let ir = parse(source);
        let markdown = render(&ir);
        let restored = parse(&markdown_html(&markdown));
        let breaks = |ir: &Ir| {
            ir.blocks
                .iter()
                .flat_map(|block| &block.runs)
                .filter(|run| run.line_break)
                .count()
        };
        assert_eq!(breaks(&ir), breaks(&restored), "{markdown}");
        let characters = |ir: &Ir| {
            ir.blocks
                .iter()
                .flat_map(|block| block.runs.iter())
                .flat_map(|run| {
                    run.text.chars().filter(|ch| *ch != '\n').map(move |ch| {
                        (
                            ch,
                            run.bold,
                            run.italic,
                            run.strike,
                            run.link.clone(),
                            run.inline_kind.map(|kind| kind.tag()),
                        )
                    })
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            characters(&ir),
            characters(&restored),
            "{source}\n{markdown}"
        );
    }
}
