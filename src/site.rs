//! Static WebPlus template selection and metadata extraction.
use crate::{
    error::{RecoverError, Result},
    rules,
};
use scraper::{Html, Selector};
use serde_json::{Map, Value};

const CONTENT: &[&str] = &[".Article_Content", ".wp_articlecontent"];

pub fn content_selector(document: &Html, requested: &str) -> Result<String> {
    if requested != "auto" {
        Selector::parse(requested).map_err(|_| RecoverError::InvalidSelector {
            selector: requested.into(),
        })?;
        return Ok(requested.into());
    }
    CONTENT
        .iter()
        .find(|candidate| {
            document
                .select(&Selector::parse(candidate).unwrap())
                .next()
                .is_some()
        })
        .map(|value| (*value).into())
        .ok_or_else(|| RecoverError::ContentNotFound {
            selector: CONTENT.join(" 或 "),
        })
}

pub fn metadata(document: &Html) -> Map<String, Value> {
    let mut result = Map::new();
    for (key, selectors, labels) in [
        ("title", &[".Article_Title", ".arti_title"][..], &[][..]),
        (
            "date",
            &[".Article_PublishDate", ".arti_update"][..],
            &["发布时间", "发布日期"][..],
        ),
        (
            "source",
            &[".Article_Source", ".arti_source"][..],
            &["文章来源", "来源"][..],
        ),
        (
            "author",
            &[".Article_Author", ".arti_author"][..],
            &["作者"][..],
        ),
        (
            "publisher",
            &[".arti_publisher"][..],
            &["发布者", "发布人"][..],
        ),
    ] {
        let value = selectors.iter().find_map(|selector| {
            document
                .select(&Selector::parse(selector).unwrap())
                .find_map(|node| {
                    let raw = rules::text(node);
                    let mut text = raw.trim();
                    for label in labels {
                        if let Some(rest) = text.strip_prefix(label) {
                            if let Some(rest) = rest.trim_start().strip_prefix([':', '：']) {
                                text = rest.trim();
                                break;
                            }
                        }
                    }
                    (!text.is_empty()).then(|| Value::String(text.into()))
                })
        });
        if let Some(value) = value {
            result.insert(key.into(), value);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_priority_and_explicit_selection() {
        let doc = Html::parse_document(
            "<div class='Article_Content'><div class='wp_articlecontent'>正文</div></div>",
        );
        assert_eq!(content_selector(&doc, "auto").unwrap(), ".Article_Content");
        assert_eq!(content_selector(&doc, ".custom").unwrap(), ".custom");
        assert!(content_selector(&doc, "[").is_err());
        assert!(content_selector(&Html::parse_document("<p>导航</p>"), "auto").is_err());
    }
    #[test]
    fn webplus_metadata_skips_empty_values_and_separates_publisher() {
        let doc = Html::parse_document("<h1 class='arti_title'></h1><h1 class='arti_title'>学院新闻</h1><span class='arti_update'>发布时间：2026-11-02</span><span class='arti_publisher'>发布者：管理员</span><span class='arti_author'>作者: 张三</span><div class='wp_articlecontent'><p>正文</p></div>");
        assert_eq!(
            content_selector(&doc, "auto").unwrap(),
            ".wp_articlecontent"
        );
        let meta = metadata(&doc);
        assert_eq!(meta["title"], "学院新闻");
        assert_eq!(meta["date"], "2026-11-02");
        assert_eq!(meta["publisher"], "管理员");
        assert_eq!(meta["author"], "张三");
    }
}
