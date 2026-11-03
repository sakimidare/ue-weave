use scraper::{ElementRef, Selector};
use serde_json::{json, Value};

pub fn identify(link: ElementRef, href: &str) -> Option<Value> {
    let path = url::Url::parse(href)
        .or_else(|_| {
            url::Url::parse("https://relative.invalid/")
                .unwrap()
                .join(href)
        })
        .ok()
        .map(|url| url.path().to_ascii_lowercase())
        .unwrap_or_default();
    let extension = [
        ".doc", ".docx", ".xls", ".xlsx", ".pdf", ".zip", ".rar", ".7z", ".ppt", ".pptx", ".odt",
        ".ods", ".txt", ".csv",
    ]
    .iter()
    .any(|extension| path.ends_with(extension));
    let icon = link
        .select(&Selector::parse("img").unwrap())
        .any(crate::rules::attachment_icon);
    let raw = link.value().attr("sudyfile-attr");
    let upload = link
        .value()
        .attr("isupload")
        .is_some_and(|value| value.eq_ignore_ascii_case("true") || value == "1");
    let download = link.value().attr("download");
    if !(extension || icon || raw.is_some() || upload || download.is_some()) {
        return None;
    }
    let mut reasons = Vec::new();
    if extension {
        reasons.push("URL 路径具有文档或归档扩展名");
    }
    if icon {
        reasons.push("链接包含附件图标");
    }
    if raw.is_some() {
        reasons.push("链接包含 sudyfile-attr");
    }
    if upload {
        reasons.push("链接带 isupload 上传标记");
    }
    if download.is_some() {
        reasons.push("链接具有 download 属性");
    }
    let parsed = raw.and_then(parse_metadata);
    Some(
        json!({"confidence":if extension || icon || raw.is_some() || download.is_some() {"high"} else {"medium"},"reason":reasons.join("；"),"attributes":{"sudyfile_raw":raw,"sudyfile_json":parsed,"isupload":link.value().attr("isupload"),"download":download,"title":link.value().attr("title")}}),
    )
}

/// Accept JSON objects plus single-quoted string literals. No expressions are evaluated.
fn parse_metadata(raw: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        return value.is_object().then_some(value);
    }
    let mut chars = raw.chars();
    let mut normalized = String::new();
    while let Some(ch) = chars.next() {
        if !matches!(ch, '\'' | '"') {
            normalized.push(ch);
            continue;
        }
        let quote = ch;
        let mut value = String::new();
        let mut closed = false;
        while let Some(ch) = chars.next() {
            if ch == quote {
                closed = true;
                break;
            }
            if ch == '\\' {
                value.push(match chars.next()? {
                    '\'' => '\'',
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    _ => return None,
                });
            } else if ch.is_control() {
                return None;
            } else {
                value.push(ch);
            }
        }
        if !closed {
            return None;
        }
        normalized.push_str(&serde_json::to_string(&value).ok()?);
    }
    let value: Value = serde_json::from_str(&normalized).ok()?;
    value.is_object().then_some(value)
}

#[cfg(test)]
mod tests {
    #[test]
    fn attributes_identify_extensionless_downloads_without_query_false_positives() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><p><a href='/search?q=a.pdf'>检索</a><a href='/a.pdf.html'>网页</a><a href='/a.PDF?token=1'>文件</a><a href='/download?id=3' sudyfile-attr=\"{'title':'附件.doc'}\" title='附件.doc'>附件</a><a href='/get?id=4' download='x.docx'>下载</a></p></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        assert_eq!(ir.attachments.len(), 3);
        assert_eq!(ir.attachments[1]["attributes"]["title"], "附件.doc");
        assert_eq!(
            ir.attachments[1]["attributes"]["sudyfile_json"]["title"],
            "附件.doc"
        );
        assert!(!ir
            .warnings
            .iter()
            .any(|w| w["kind"] == "attachment-metadata-unparsed"));
    }
    #[test]
    fn metadata_parser_rejects_expressions_and_handles_quoted_text() {
        assert_eq!(
            super::parse_metadata(r#"{'title':'O\'Brien "Notes".doc','size':42}"#).unwrap()
                ["title"],
            "O'Brien \"Notes\".doc"
        );
        for raw in [
            "{'title':alert(1)}",
            "{'title':function(){}}",
            "{'title':'unterminated}",
            "['x']",
        ] {
            assert!(super::parse_metadata(raw).is_none());
        }
    }
}
