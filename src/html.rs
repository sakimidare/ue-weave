//! 按静态页面解析 noscript 中的备用内容。
pub fn parse(source: &str) -> scraper::Html {
    use tendril::TendrilSink;
    let mut options = html5ever::ParseOpts::default();
    options.tree_builder.scripting_enabled = false;
    html5ever::parse_document(
        scraper::HtmlTreeSink::new(scraper::Html::new_document()),
        options,
    )
    .one(source)
}
