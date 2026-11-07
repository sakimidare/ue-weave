//! URL 和本地 HTML 共用的选择器配置。
use crate::{
    error::{RecoverError, Result},
    site,
};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{fs, path::Path};

/// 正文与元数据字段的 CSS 选择器，等价于命令行的 `--selector` 和 `--site-config`。
///
/// 未设置的字段使用内置模板规则；`content` 为 `None` 时自动识别正文。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SiteSelectors {
    content: Option<String>,
    title: Option<String>,
    date: Option<String>,
    source: Option<String>,
    author: Option<String>,
    publisher: Option<String>,
}

impl SiteSelectors {
    /// 全部字段使用内置规则。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 JSON 对象读取，等价于 `--site-config`；未知字段会被拒绝。
    pub fn from_json(bytes: &[u8]) -> std::result::Result<Self, crate::Error> {
        Self::parse(bytes, Path::new("<json>")).map_err(crate::Error::from)
    }

    /// 正文选择器；设为 `auto` 与省略等价。
    pub fn content(mut self, selector: impl Into<String>) -> Self {
        self.content = Some(selector.into());
        self
    }

    /// 标题选择器。
    pub fn title(mut self, selector: impl Into<String>) -> Self {
        self.title = Some(selector.into());
        self
    }

    /// 日期选择器。
    pub fn date(mut self, selector: impl Into<String>) -> Self {
        self.date = Some(selector.into());
        self
    }

    /// 来源选择器。
    pub fn source(mut self, selector: impl Into<String>) -> Self {
        self.source = Some(selector.into());
        self
    }

    /// 作者选择器。
    pub fn author(mut self, selector: impl Into<String>) -> Self {
        self.author = Some(selector.into());
        self
    }

    /// 发布者选择器。
    pub fn publisher(mut self, selector: impl Into<String>) -> Self {
        self.publisher = Some(selector.into());
        self
    }

    pub(crate) fn content_selector(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub(crate) fn apply_overrides(&mut self, overrides: &[String]) -> Result<Option<String>> {
        let mut content_override = None;
        for value in overrides {
            let (field, css) = match value.split_once('=') {
                Some((field, css))
                    if !field.trim().is_empty()
                        && field
                            .trim()
                            .chars()
                            .all(|c| c.is_ascii_alphabetic() || c == '_') =>
                {
                    (field.trim(), css.trim())
                }
                _ => ("content", value.trim()),
            };
            let target = match field {
                "content" => &mut self.content,
                "title" => &mut self.title,
                "date" => &mut self.date,
                "source" => &mut self.source,
                "author" => &mut self.author,
                "publisher" => &mut self.publisher,
                _ => return Err(RecoverError::Arguments(format!("未知选择器字段：{field}；可用字段：content、title、date、source、author、publisher"))),
            };
            if field != "content" || css != "auto" {
                Selector::parse(css).map_err(|_| RecoverError::InvalidSelector {
                    selector: format!("{field}={css}"),
                })?;
            }
            *target = Some(css.into());
            if field == "content" {
                content_override = Some(css.into());
            }
        }
        Ok(content_override)
    }
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).map_err(|source| RecoverError::ReadConfig {
            path: path.into(),
            source,
        })?;
        Self::parse(&bytes, path)
    }

    fn parse(bytes: &[u8], path: &Path) -> Result<Self> {
        let config: Self = serde_json::from_slice(bytes).map_err(|error| RecoverError::Config {
            path: path.into(),
            reason: error.to_string(),
        })?;
        for (field, selector) in [
            ("content", &config.content),
            ("title", &config.title),
            ("date", &config.date),
            ("source", &config.source),
            ("author", &config.author),
            ("publisher", &config.publisher),
        ] {
            if let Some(selector) = selector {
                if field == "content" && selector == "auto" {
                    continue;
                }
                Selector::parse(selector).map_err(|_| RecoverError::Config {
                    path: path.into(),
                    reason: format!("{field} 的 CSS 选择器无效：{selector}"),
                })?;
            }
        }
        Ok(config)
    }

    pub(crate) fn apply_metadata(&self, document: &Html, metadata: &mut Map<String, Value>) {
        for (key, selector, labels) in [
            ("title", &self.title, &[][..]),
            ("date", &self.date, &["发布时间", "发布日期"][..]),
            ("source", &self.source, &["文章来源", "来源"][..]),
            ("author", &self.author, &["作者"][..]),
            ("publisher", &self.publisher, &["发布者", "发布人"][..]),
        ] {
            if let Some(selector) = selector {
                metadata.remove(key);
                if let Some(value) = site::extract(document, selector, labels) {
                    metadata.insert(key.into(), value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_fields_override_individually_and_preserve_attribute_equals() {
        let mut selectors: SiteSelectors =
            serde_json::from_value(serde_json::json!({"title":"h1","date":"time"})).unwrap();
        selectors
            .apply_overrides(&[
                "title=h2".into(),
                "title=.headline".into(),
                "article[data-type='news']".into(),
                "author=.byline".into(),
                "source=.origin".into(),
                "publisher=.editor".into(),
            ])
            .unwrap();
        assert_eq!(selectors.title.as_deref(), Some(".headline"));
        assert_eq!(
            selectors.content.as_deref(),
            Some("article[data-type='news']")
        );
        assert_eq!(selectors.date.as_deref(), Some("time"));
        assert_eq!(selectors.author.as_deref(), Some(".byline"));
        assert_eq!(selectors.source.as_deref(), Some(".origin"));
        assert_eq!(selectors.publisher.as_deref(), Some(".editor"));
        assert!(selectors.apply_overrides(&["titel=h1".into()]).is_err());
        assert!(selectors.apply_overrides(&["date=[".into()]).is_err());
    }
    #[test]
    fn partial_configuration_preserves_automatic_metadata() {
        let doc = Html::parse_document("<h1 class='arti_title'>原始标题</h1><span class='arti_update'>发布时间：2026-11-03</span><h2>自定义标题</h2>\
            <div class='wp_articlecontent'>正文</div>");
        let mut meta = site::metadata(&doc);
        let selectors: SiteSelectors =
            serde_json::from_value(serde_json::json!({"title":"h2"})).unwrap();
        selectors.apply_metadata(&doc, &mut meta);
        assert_eq!(meta["title"], "自定义标题");
        assert_eq!(meta["date"], "2026-11-03");
    }
    #[test]
    fn explicit_missing_metadata_stays_absent() {
        let doc = Html::parse_document(
            "<h1 class='arti_title'>默认标题</h1><div class='wp_articlecontent'>正文</div>",
        );
        let mut meta = site::metadata(&doc);
        let selectors: SiteSelectors =
            serde_json::from_value(serde_json::json!({"title":".missing"})).unwrap();
        selectors.apply_metadata(&doc, &mut meta);
        assert!(!meta.contains_key("title"));
    }
}
