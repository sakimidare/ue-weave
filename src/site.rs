//! 静态识别 WebPlus 模板并提取文章信息。
use crate::{
    error::{RecoverError, Result},
    rules,
};
use scraper::{Html, Selector};
use serde_json::{Map, Value};

const CONTENT: &[&str] = &[".Article_Content", ".wp_articlecontent"];

pub fn content_selector(document: &Html, requested: &str) -> Result<String> {
    if let Some(node) = document
        .select(&Selector::parse(".wp_error_msg").unwrap())
        .next()
    {
        let message = rules::text(node).trim().to_owned();
        if ["校内", "权限", "登录", "访问受限"]
            .iter()
            .any(|part| message.contains(part))
        {
            return Err(RecoverError::AccessRestricted { message });
        }
    }
    if requested != "auto" {
        Selector::parse(requested).map_err(|_| RecoverError::InvalidSelector {
            selector: requested.into(),
        })?;
        check_empty(document, requested)?;
        return Ok(requested.into());
    }
    let selector = CONTENT
        .iter()
        .find(|candidate| {
            document
                .select(&Selector::parse(candidate).unwrap())
                .next()
                .is_some()
        })
        .map(|value| (*value).to_owned())
        .ok_or_else(|| RecoverError::ContentNotFound {
            selector: CONTENT.join(" 或 "),
        });
    match selector {
        Ok(selector) => {
            check_empty(document, &selector)?;
            Ok(selector)
        }
        Err(error) => {
            check_empty(document, "article.detail-body")?;
            Err(error)
        }
    }
}

fn check_empty(document: &Html, selector: &str) -> Result<()> {
    if let Some(root) = document.select(&Selector::parse(selector).unwrap()).next() {
        let has_text = root
            .text()
            .any(|text| text.chars().any(|ch| !ch.is_whitespace()));
        let has_resource = root.select(&Selector::parse("img,a[href],iframe,video,audio,object,embed,svg,math,[sudyfile-attr],[pdfsrc],[src]").unwrap()).next().is_some();
        if !has_text && !has_resource {
            return Err(RecoverError::EmptyContent {
                selector: selector.into(),
            });
        }
    }
    Ok(())
}

pub fn metadata(document: &Html) -> Map<String, Value> {
    let mut result = Map::new();
    let college_template = document
        .select(&Selector::parse(".wp_articlecontent").unwrap())
        .next()
        .is_some()
        && document
            .select(&Selector::parse(".Article_Content").unwrap())
            .next()
            .is_none();
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
        let value = selectors
            .iter()
            .filter(|selector| !college_template || selector.starts_with(".arti_"))
            .find_map(|selector| extract(document, selector, labels));
        if let Some(value) = value {
            result.insert(key.into(), value);
        }
    }
    result
}

pub fn extract(document: &Html, selector: &str, labels: &[&str]) -> Option<Value> {
    document
        .select(&Selector::parse(selector).ok()?)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_restricted_empty_and_missing_pages() {
        for (html, code) in [
            (
                "<div class='wp_error_msg'>提示：仅允许校内地址访问</div>",
                "E0015",
            ),
            (
                "<article class='detail-body'><div> </div></article>",
                "E0016",
            ),
            (
                "<div class='Article_Content'><p>&nbsp;<br></p></div>",
                "E0016",
            ),
            ("<p>其他模板</p>", "E0007"),
        ] {
            assert_eq!(
                content_selector(&Html::parse_document(html), "auto")
                    .unwrap_err()
                    .code(),
                code
            );
        }
        for html in [
            "<img src='a.png'>",
            "<a href='x.pdf'>附件</a>",
            "<iframe src='a'></iframe>",
        ] {
            let doc = Html::parse_document(&format!("<div class='Article_Content'>{html}</div>"));
            assert!(content_selector(&doc, "auto").is_ok());
        }
    }
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
        let doc = Html::parse_document("<h1 class='arti_title'></h1><h1 class='arti_title'>学院新闻</h1><span class='arti_update'>\
            发布时间：2026-11-02</span><span class='arti_publisher'>发布者：管理员</span><span class='arti_author'>作者: \
            张三</span><div class='wp_articlecontent'><p>正文</p></div>");
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
