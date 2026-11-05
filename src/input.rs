use crate::diagnostic::{Details, Diagnostic, WarningKind};
use encoding_rs::{Encoding, UTF_8};
use scraper::{Html, Selector};

pub struct Input {
    pub bytes: Vec<u8>,
    pub html: String,
    pub source: String,
    pub warnings: Vec<Diagnostic>,
}

pub fn charset(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['\'', '"']).to_string())
    })
}

impl Input {
    pub fn decode(bytes: Vec<u8>, source: String, declared: Option<String>) -> Self {
        let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]);
        let meta = Html::parse_document(&prefix);
        let label = declared.or_else(|| {
            meta.select(&Selector::parse("meta").unwrap())
                .find_map(|node| {
                    node.value()
                        .attr("charset")
                        .map(str::to_string)
                        .or_else(|| {
                            node.value()
                                .attr("http-equiv")
                                .filter(|value| value.eq_ignore_ascii_case("content-type"))?;
                            charset(node.value().attr("content")?)
                        })
                })
        });
        let mut warnings = Vec::new();
        let encoding = Encoding::for_bom(&bytes)
            .map(|(encoding, _)| encoding)
            .or_else(|| {
                label
                    .as_deref()
                    .and_then(|label| Encoding::for_label(label.as_bytes()))
            })
            .unwrap_or(UTF_8);
        if label
            .as_deref()
            .is_some_and(|label| Encoding::for_label(label.as_bytes()).is_none())
        {
            warnings.push(Diagnostic::new(
                WarningKind::UnknownEncoding,
                Details {
                    original: serde_json::json!(label),
                    value: serde_json::json!(encoding.name()),
                    confidence: Some("low".to_owned()),
                    reason: Some(
                        ("编码声明未匹配已支持的编码，使用 BOM 指定的编码或 UTF-8").to_owned(),
                    ),
                    ..Default::default()
                },
            ));
        }
        let (text, _, replaced) = encoding.decode(&bytes);
        let html = text.into_owned();
        if replaced {
            warnings.push(Diagnostic::new(
                WarningKind::DecodeReplacement,
                Details {
                    value: serde_json::json!(encoding.name()),
                    confidence: Some("high".to_owned()),
                    reason: Some("部分输入字节无法按检测到的编码解码，已使用替换字符".to_owned()),
                    ..Default::default()
                },
            ));
        }
        Self {
            bytes,
            html,
            source,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_gbk_and_preserves_source_bytes() {
        let source = "<meta charset='gbk'><p>中文&nbsp;</p>";
        let (bytes, _, _) = encoding_rs::GBK.encode(source);
        let input = Input::decode(bytes.to_vec(), "fixture".into(), None);
        assert_eq!(input.html, source);
        assert_eq!(input.bytes, bytes.as_ref());
        assert!(input.warnings.is_empty());
    }
    #[test]
    fn reports_invalid_bytes_without_losing_original() {
        let input = Input::decode(vec![0xff, 0x41], "fixture".into(), Some("UTF-8".into()));
        assert_eq!(input.bytes, [0xff, 0x41]);
        assert_eq!(input.warnings[0]["kind"], "decode-replacement");
    }

    #[test]
    fn finds_charset_after_the_legacy_four_kib_limit() {
        let mut source = "<head>".to_string();
        source.push_str(&" ".repeat(5000));
        source.push_str("<meta charset='gbk'></head><p>中文</p>");
        let (bytes, _, _) = encoding_rs::GBK.encode(&source);
        let input = Input::decode(bytes.to_vec(), "fixture".into(), None);
        assert_eq!(input.html, source);
        assert!(input.warnings.is_empty());
    }
}
