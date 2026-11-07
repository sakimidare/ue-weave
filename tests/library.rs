use ue_weave::{Article, Level, Options, OutputOptions, Profile};

const PAGE: &str = "<div class='body'><p>请<strong>按时</strong>报名</p><p><a href='/form.pdf'>申请表</a></p></div>";

#[test]
fn parses_explicit_selector_and_exports_every_format() {
    let article = Article::from_html(
        PAGE,
        Options::new()
            .selector(".body")
            .source("https://example.org/news/page.html"),
    )
    .unwrap();
    assert!(article.to_html().contains("<strong>按时</strong>"));
    assert!(article.to_markdown().contains("**按时**"));
    assert!(article.to_typst().contains("申请表"));
    assert!(article.to_html().contains("https://example.org/form.pdf"));
    assert_eq!(article.to_json()["blocks"].as_array().unwrap().len(), 2);
    assert_eq!(
        article.metadata().url.as_deref(),
        Some("https://example.org/news/page.html")
    );
}

#[test]
fn auto_detects_the_content_container() {
    let page = "<div class='Article_Content'><p>教务处通知</p></div>";
    let article = Article::from_html(page, Options::new()).unwrap();
    assert!(article.to_markdown().contains("教务处通知"));
}

#[test]
fn decodes_declared_encoding_from_bytes() {
    let mut bytes = b"<meta charset='gbk'><div class='Article_Content'><p>".to_vec();
    bytes.extend_from_slice(&[0xd6, 0xd0, 0xce, 0xc4]);
    bytes.extend_from_slice(b"</p></div>");
    let article = Article::from_bytes(&bytes, Options::new()).unwrap();
    assert!(article.to_markdown().contains("中文"));
}

#[test]
fn json_round_trips_through_from_json() {
    let article = Article::from_html(PAGE, Options::new().selector(".body")).unwrap();
    let json = article.to_json();
    let restored = Article::from_json(json.clone()).unwrap();
    assert_eq!(restored.to_markdown(), article.to_markdown());
    assert_eq!(restored.to_json(), json);
}

#[test]
fn metadata_and_assets_are_typed() {
    let page = "<span class='Article_Title'>通知</span><span class='Article_PublishDate'>2026-10-14</span>\
        <div class='Article_Content'><p>正文</p><img src='/a.png' alt='示意图'>\
        <a href='/b.docx' sudyfile-attr=\"{'title':'附件.docx'}\">附件</a></div>";
    let article = Article::from_html(
        page,
        Options::new().source("https://example.org/a/page.htm"),
    )
    .unwrap();
    assert_eq!(article.metadata().title.as_deref(), Some("通知"));
    assert_eq!(article.metadata().date.as_deref(), Some("2026-10-14"));
    let images = article.images();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].url.as_deref(), Some("https://example.org/a.png"));
    assert_eq!(images[0].alt.as_deref(), Some("示意图"));
    let attachments = article.attachments();
    assert_eq!(attachments.len(), 1);
    assert!(attachments[0].url.as_deref().unwrap().ends_with("b.docx"));
}

#[test]
fn diagnostics_expose_codes_and_messages() {
    let page =
        "<div class='Article_Content'><p><span style='letter-spacing:2px'>甲</span></p></div>";
    let article = Article::from_html(page, Options::new()).unwrap();
    let diagnostics = article.diagnostics();
    let unmapped = diagnostics
        .iter()
        .find(|item| item.code() == "W0014")
        .expect("missing W0014");
    assert_eq!(unmapped.level(), Level::Warning);
    assert_eq!(unmapped.kind(), "unmapped-style");
    assert!(unmapped.message().contains("样式"));
    assert_eq!(unmapped.details()["attribute"], "letter-spacing");
}

#[test]
fn markdown_profile_switches_tables_to_native_markdown() {
    let page = "<div class='Article_Content'><table><tr><th>甲</th><th>乙</th></tr>\
        <tr><td>1</td><td>2</td></tr></table></div>";
    let article = Article::from_html(page, Options::new()).unwrap();
    assert!(article.to_markdown().contains("<table"));
    let aggressive = article.to_markdown_with(OutputOptions::new(Profile::Aggressive));
    assert!(!aggressive.contains("<table"), "{aggressive}");
    assert!(aggressive.contains("| 甲 |"), "{aggressive}");
}

#[test]
fn deprecated_names_match_the_new_ones() {
    #[allow(deprecated)]
    {
        let article = Article::from_html(PAGE, Options::new().selector(".body")).unwrap();
        assert_eq!(article.html(), article.to_html());
        assert_eq!(article.markdown(), article.to_markdown());
        assert_eq!(article.typst(), article.to_typst());
        assert_eq!(article.json(), article.to_json());
    }
}

#[test]
fn library_output_matches_cli_profiles() {
    let bytes = std::fs::read("fixtures/seu-585911.html").unwrap();
    let article = Article::from_bytes(&bytes, Options::new()).unwrap();
    for profile in [Profile::Conservative, Profile::Aggressive] {
        for metadata in [false, true] {
            let dir = tempfile::Builder::new()
                .prefix("ue-lib-compare-")
                .tempdir()
                .unwrap();
            let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_ue-weave"));
            command
                .args(["--input", "fixtures/seu-585911.html", "--output"])
                .arg(dir.path())
                .args(["--formats", "html,markdown,typst", "--profile"])
                .arg(match profile {
                    Profile::Conservative => "conservative",
                    Profile::Balanced => "balanced",
                    Profile::Aggressive => "aggressive",
                    _ => unreachable!("仅三种预设"),
                });
            if metadata {
                command.arg("--include-metadata");
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{profile:?}");
            let expected = article.to_document(OutputOptions {
                include_metadata: Some(metadata),
                ..OutputOptions::new(profile)
            });
            for (name, actual) in [
                ("reconstructed.html", expected.html),
                ("reconstructed.md", expected.markdown),
                ("reconstructed.typ", expected.typst),
            ] {
                let cli = std::fs::read_to_string(dir.path().join(name)).unwrap();
                assert_eq!(actual, cli, "{profile:?} metadata={metadata} {name}");
            }
        }
    }
}

#[test]
fn single_format_helpers_match_to_document() {
    let bytes = std::fs::read("fixtures/seu-585911.html").unwrap();
    let article = Article::from_bytes(&bytes, Options::new()).unwrap();
    for profile in [
        Profile::Conservative,
        Profile::Balanced,
        Profile::Aggressive,
    ] {
        let document = article.to_document(OutputOptions::new(profile));
        assert_eq!(
            article.to_html_with(OutputOptions::new(profile)),
            document.html
        );
        assert_eq!(
            article.to_markdown_with(OutputOptions::new(profile)),
            document.markdown
        );
        assert_eq!(
            article.to_typst_with(OutputOptions::new(profile)),
            document.typst
        );
    }
    let default = article.to_document(OutputOptions::default());
    assert_eq!(article.to_html(), default.html);
    assert_eq!(article.to_markdown(), default.markdown);
    assert_eq!(article.to_typst(), default.typst);
}

#[test]
fn output_options_match_cli_overrides() {
    let bytes = std::fs::read("fixtures/seu-585911.html").unwrap();
    let article = Article::from_bytes(&bytes, Options::new()).unwrap();
    let cases: [(&str, OutputOptions, &[&str]); 5] = [
        (
            "aggressive keeps fonts",
            OutputOptions {
                profile: Profile::Aggressive,
                strip_fonts: Some(false),
                ..OutputOptions::default()
            },
            &["--profile", "aggressive", "--strip-fonts=false"],
        ),
        (
            "aggressive keeps html tables",
            OutputOptions {
                profile: Profile::Aggressive,
                native_markdown: Some(false),
                ..OutputOptions::default()
            },
            &["--profile", "aggressive", "--native-markdown=false"],
        ),
        (
            "conservative strips fonts",
            OutputOptions {
                strip_fonts: Some(true),
                ..OutputOptions::default()
            },
            &["--strip-fonts"],
        ),
        (
            "balanced keeps leading spaces",
            OutputOptions {
                profile: Profile::Balanced,
                trim_leading_spaces: Some(false),
                ..OutputOptions::default()
            },
            &["--profile", "balanced", "--trim-leading-spaces=false"],
        ),
        (
            "conservative keeps broken fonts",
            OutputOptions {
                repair_corrupt_fonts: Some(false),
                ..OutputOptions::default()
            },
            &["--no-repair-corrupt-fonts"],
        ),
    ];
    for (label, options, args) in cases {
        let dir = tempfile::Builder::new()
            .prefix("ue-lib-override-")
            .tempdir()
            .unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["--input", "fixtures/seu-585911.html", "--output"])
            .arg(dir.path())
            .args(["--formats", "html,markdown,typst"])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{label}");
        let expected = article.to_document(options);
        for (name, actual) in [
            ("reconstructed.html", expected.html),
            ("reconstructed.md", expected.markdown),
            ("reconstructed.typ", expected.typst),
        ] {
            let cli = std::fs::read_to_string(dir.path().join(name)).unwrap();
            assert_eq!(actual, cli, "{label}: {name}");
        }
    }
}

#[test]
fn errors_have_the_same_codes_as_the_cli() {
    for (options, expected) in [
        (Options::new().selector("["), "E0006"),
        (Options::new().selector(".missing"), "E0007"),
    ] {
        let error = Article::from_html("<p>通知</p>", options).err().unwrap();
        assert_eq!(error.code(), expected);
    }
    let error = Article::from_json_bytes(b"{\"blocks\":[]}").err().unwrap();
    assert!(error.code().starts_with('E'));
}

#[test]
fn errors_expose_their_source_chain() {
    use std::error::Error as _;
    let error = Article::from_json_bytes(b"{").err().unwrap();
    assert_eq!(error.code(), "E0012");
    assert!(error.source().is_some());
}

#[test]
fn document_reports_export_changes() {
    let page = "<div class='Article_Content'><p><span style=\"font-family:微软雅黑, undefinedMicrosoft YaHeiundefined\">甲</span></p>\
        <table><tr><td>a</td><td>b</td></tr></table></div>";
    let article = Article::from_html(page, Options::new()).unwrap();
    let document = article.to_document(OutputOptions {
        profile: Profile::Aggressive,
        ..OutputOptions::default()
    });
    let kinds: Vec<_> = document
        .changes
        .iter()
        .map(|change| change.kind())
        .collect();
    assert!(kinds.contains(&"corrupt-font-repair"), "{kinds:?}");
    assert!(kinds.contains(&"markdown-table-header"), "{kinds:?}");
    let repair = document
        .changes
        .iter()
        .find(|change| change.kind() == "corrupt-font-repair")
        .unwrap();
    assert_eq!(repair.field(), Some("runs.font_family"));
    assert_eq!(repair.details()["value"], "微软雅黑, \"Microsoft YaHei\"");
}

#[test]
fn diagnostics_expose_log_topics() {
    let page = "<div class='Article_Content'><p><span style='letter-spacing:2px'>甲</span></p>\
        <p><img class='sudy-attachment-icon' src='/_ueditor/a.gif'></p></div>";
    let article = Article::from_html(page, Options::new()).unwrap();
    let diagnostics = article.diagnostics();
    let style = diagnostics
        .iter()
        .find(|item| item.code() == "W0014")
        .unwrap();
    assert_eq!(style.topics(), ["style"].as_slice());
    let icon = diagnostics
        .iter()
        .find(|item| item.code() == "N0007")
        .unwrap();
    assert_eq!(icon.topics(), ["attachment"].as_slice());
}

#[test]
fn byte_order_mark_wins_over_meta_charset() {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(
        "<meta charset='gbk'><div class='Article_Content'><p>中文</p></div>".as_bytes(),
    );
    let article = Article::from_bytes(&bytes, Options::new()).unwrap();
    assert!(article.to_markdown().contains("中文"));
}

#[test]
fn compact_json_round_trips_without_source_nodes() {
    let page = "<div class='Article_Content'><p>正文<strong>加粗</strong></p>\
        <p><a href='/a.pdf'>附件</a></p></div>";
    let article = Article::from_html(page, Options::new()).unwrap();
    let full = article.to_json_bytes();
    let compact = article.to_json_compact_bytes();
    assert!(
        compact.len() < full.len(),
        "{} vs {}",
        compact.len(),
        full.len()
    );

    let value: serde_json::Value = serde_json::from_slice(&compact).unwrap();
    assert!(value.get("original_nodes").is_none());
    assert!(value["blocks"].to_string().contains("正文"));

    let restored = Article::from_json_bytes(&compact).unwrap();
    assert_eq!(restored.to_markdown(), article.to_markdown());
    assert_eq!(restored.diagnostics().len(), article.diagnostics().len());
}

#[test]
fn downloaded_assets_keep_the_original_url() {
    let document = serde_json::json!({
        "schema": {"name": "ue-weave-ir", "version": 2},
        "metadata": {},
        "blocks": [],
        "images": [{
            "src": "images/0000-a.jpg",
            "remote_src": "https://example.org/a.png",
            "local_path": "images/0000-a.jpg",
            "alt": "示意图"
        }],
        "attachments": [{
            "href": "attachments/0000-b.pdf",
            "remote_href": "https://example.org/b.pdf",
            "local_path": "attachments/0000-b.pdf",
            "text": "申请表"
        }]
    });
    let article = Article::from_json(document).unwrap();

    let image = &article.images()[0];
    assert_eq!(image.url.as_deref(), Some("https://example.org/a.png"));
    assert_eq!(image.local_path.as_deref(), Some("images/0000-a.jpg"));

    let attachment = &article.attachments()[0];
    assert_eq!(attachment.url.as_deref(), Some("https://example.org/b.pdf"));
    assert_eq!(
        attachment.local_path.as_deref(),
        Some("attachments/0000-b.pdf")
    );

    let exported = article.to_json();
    assert_eq!(
        exported["images"][0]["remote_src"],
        "https://example.org/a.png"
    );
    assert_eq!(
        exported["attachments"][0]["remote_href"],
        "https://example.org/b.pdf"
    );
}
