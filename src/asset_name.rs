//! 从响应及来源地址提取文件名，并整理为跨平台可用的路径部分。
use url::Url;

fn decode(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .into_owned()
}

fn parameters(header: &str) -> Vec<&str> {
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut result = Vec::new();
    for (index, ch) in header.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && quoted {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if ch == ';' && !quoted {
            result.push(&header[start..index]);
            start = index + 1;
        }
    }
    result.push(&header[start..]);
    result
}

pub fn proposed(
    disposition: Option<&str>,
    title: Option<&str>,
    url: &str,
) -> (String, &'static str) {
    if let Some(header) = disposition {
        let mut plain = None;
        for parameter in parameters(header) {
            let Some((name, value)) = parameter.trim().split_once('=') else {
                continue;
            };
            let name = name.trim();
            let value = value.trim().trim_matches('"');
            if name.eq_ignore_ascii_case("filename*") {
                if let Some((encoding, rest)) = value.split_once('\'') {
                    if encoding.eq_ignore_ascii_case("utf-8") {
                        if let Some((_, encoded)) = rest.split_once('\'') {
                            let name = decode(encoded);
                            if !name.is_empty() {
                                return (name, "content-disposition");
                            }
                        }
                    }
                }
            } else if name.eq_ignore_ascii_case("filename") && !value.is_empty() {
                plain = Some(value.replace("\\\"", "\"").replace("\\\\", "\\"));
            }
        }
        if let Some(name) = plain {
            return (name, "content-disposition");
        }
    }
    if let Some(title) = title.filter(|title| std::path::Path::new(title).extension().is_some()) {
        return (title.into(), "attachment-title");
    }
    let name = Url::parse(url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut parts| parts.next_back())
                .map(decode)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "attachment".into());
    (name, "url")
}

pub fn filename(proposed: &str, extension: &str, index: usize) -> String {
    let stem = proposed
        .rsplit_once('.')
        .filter(|(_, ext)| {
            !ext.is_empty() && ext.len() <= 16 && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
        })
        .map(|(stem, _)| stem)
        .unwrap_or(proposed);
    let mut safe = String::new();
    for ch in stem.chars() {
        let ch = if ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            ch
        } else {
            '_'
        };
        if safe.len() + ch.len_utf8() > 180 {
            break;
        }
        safe.push(ch);
    }
    let safe = safe.trim_matches('.');
    format!(
        "{index:04}-{}.{}",
        if safe.is_empty() { "attachment" } else { safe },
        extension
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn response_names_unicode_and_path_components() {
        assert_eq!(
            super::proposed(
                Some("attachment; filename=old.docx; filename*=UTF-8''%E9%80%9A%E7%9F%A5.docx"),
                None,
                "https://a/download"
            )
            .0,
            "通知.docx"
        );
        assert_eq!(
            super::proposed(
                Some("attachment; filename=\"a;b.pdf\""),
                None,
                "https://a/download"
            )
            .0,
            "a;b.pdf"
        );
        assert_eq!(
            super::filename("../../通知.docx", "docx", 0),
            "0000-_.._通知.docx"
        );
        assert!(super::filename(&"中".repeat(200), "pdf", 1).len() < 255);
    }
}
