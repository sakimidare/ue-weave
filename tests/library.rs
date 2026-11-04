use ue_weave::Article;

#[test]
fn library_parses_and_exports_without_a_process_or_network() {
    let article = Article::from_html(
        "<div class='body'><p>请<strong>按时</strong>报名</p><p><a href='/form.pdf'>申请表</a></p></div>",
        ".body", Some("https://example.org/news/page.html"), true,
    ).unwrap();
    assert!(article.html().contains("<strong>按时</strong>"));
    assert!(article.markdown().contains("**按时**"));
    assert!(article.typst().contains("申请表"));
    assert!(article.html().contains("https://example.org/form.pdf"));
    assert_eq!(article.json()["blocks"].as_array().unwrap().len(), 2);
}

#[test]
fn library_errors_have_the_same_codes_as_the_cli() {
    for (selector, expected) in [("[", "E0006"), (".missing", "E0007")] {
        let error = Article::from_html("<p>通知</p>", selector, None, true)
            .err()
            .unwrap();
        assert_eq!(error.code(), expected);
        assert!(error.to_string().contains(selector));
    }
}
