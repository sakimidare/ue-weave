use encoding_rs::{Encoding, UTF_8};
use scraper::{Html, Selector};
use serde_json::{json, Value};

pub struct Input {
    pub bytes: Vec<u8>,
    pub html: String,
    pub source: String,
    pub warnings: Vec<Value>,
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
        let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
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
            warnings.push(json!({"kind":"unknown-encoding","original":label,"value":encoding.name(),"confidence":"low","reason":"编码声明无法识别，保留原始输入并采用 BOM 或 UTF-8","source_nodes":[]}));
        }
        let (text, _, replaced) = encoding.decode(&bytes);
        let html = text.into_owned();
        if replaced {
            warnings.push(json!({"kind":"decode-replacement","value":encoding.name(),"confidence":"high","reason":"部分字节无法解码；完整原始字节保存在 source.bin","source_nodes":[]}));
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
}
