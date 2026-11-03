use scraper::{ElementRef, Selector};
use serde_json::{json, Value};

pub fn embedded_pdf(element: ElementRef) -> Option<(String, String)> {
    if !matches!(element.value().name(), "span" | "div") {
        return None;
    }
    if !element
        .value()
        .classes()
        .any(|class| class == "wp_pdf_player")
    {
        return None;
    }
    let href = element.value().attr("pdfsrc")?;
    if !crate::safety::safe_url(href, false) {
        return None;
    }
    let text = crate::rules::text(element);
    let title = if !text.trim().is_empty() {
        text
    } else {
        element
            .value()
            .attr("sudyfile-attr")
            .and_then(parse_metadata)
            .and_then(|value| value["title"].as_str().map(str::to_string))
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "PDF 附件".into())
    };
    let title = crate::rules::clean_sentinel(&title);
    Some((
        href.into(),
        if title.trim().is_empty() {
            "PDF 附件".into()
        } else {
            title
        },
    ))
}

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
pub(crate) fn parse_metadata(raw: &str) -> Option<Value> {
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
    fn div_pdf_body_survives_profiles_and_integrity_detects_its_loss() {
        let doc = scraper::Html::parse_document(include_str!("../fixtures/webplus-div-pdf.html"));
        let ir = crate::parser::parse(
            &doc,
            ".Article_Content",
            false,
            Some("https://example.org/article".into()),
        )
        .unwrap();
        assert_eq!(ir.attachments.len(), 1);
        assert_eq!(
            ir.attachments[0]["href"],
            "https://example.org/files/news.pdf"
        );
        assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
        assert!(
            ir.metadata["content_integrity"]["original_characters"]
                .as_u64()
                .unwrap()
                > 0
        );
        let nodes = &ir.attachments[0]["source_nodes"];
        assert!(!ir
            .warnings
            .iter()
            .any(|w| w["kind"] == "empty-element" && &w["source_nodes"] == nodes));
        for profile in [
            crate::profile::Profile::Conservative,
            crate::profile::Profile::Balanced,
            crate::profile::Profile::Aggressive,
        ] {
            let output = crate::profile::prepare(&ir, profile.settings());
            let documents =
                crate::export::render(&output.ir, profile.settings(), &Default::default());
            for text in [&documents.html, &documents.markdown, &documents.typst] {
                assert!(
                    text.contains("https://example.org/files/news.pdf"),
                    "{text}"
                );
            }
            assert_eq!(documents.html.matches("href=").count(), 1);
        }
        let root = doc
            .select(&scraper::Selector::parse(".Article_Content").unwrap())
            .next()
            .unwrap();
        let mut damaged = ir.clone();
        damaged.blocks.clear();
        crate::integrity::check(root, &mut damaged);
        assert_eq!(damaged.metadata["content_integrity"]["value"], "mismatch");
    }

    #[test]
    fn div_pdf_with_fallback_content_is_atomic_and_safe() {
        let doc = scraper::Html::parse_document("<div class='Article_Content'><p>前</p><div class='wp_pdf_player' pdfsrc='/a.pdf'><p>下载 PDF</p></div><p>后</p><div class='wp_pdf_player' pdfsrc='javascript:alert(1)'>说明</div></div>");
        let ir = crate::parser::parse(&doc, ".Article_Content", false, None).unwrap();
        let html = crate::renderer::render(&ir);
        assert_eq!(ir.attachments.len(), 1);
        assert_eq!(html.matches("下载 PDF").count(), 1);
        assert!(html.contains("说明"));
        assert!(!html.contains("javascript:"));
        assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
    }

    #[test]
    fn embedded_pdf_keeps_links_and_rejects_unsafe_sources() {
        let doc = scraper::Html::parse_document("<div class='Article_Content'><p><span class='wp_pdf_player' pdfsrc='/file.pdf' sudyfile-attr=\"{'title':'undefinedundefined'}\"></span></p><p><span class='wp_pdf_player' pdfsrc='javascript:alert(1)'>保留说明</span></p></div>");
        let ir = crate::parser::parse(
            &doc,
            ".Article_Content",
            false,
            Some("https://example.org/article".into()),
        )
        .unwrap();
        assert_eq!(ir.attachments.len(), 1);
        assert_eq!(ir.blocks[0].runs[0].text, "PDF 附件");
        assert_eq!(
            ir.blocks[0].runs[0].link.as_deref(),
            Some("https://example.org/file.pdf")
        );
        assert_eq!(ir.metadata["content_integrity"]["value"], "matched");
        let html = crate::renderer::render(&ir);
        assert!(html.contains("保留说明"));
        assert!(!html.contains("javascript:"));
    }
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
