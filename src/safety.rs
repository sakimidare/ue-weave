/// Accept web links, email links and relative paths. Reject executable schemes.
pub fn safe_url(value: &str, image: bool) -> bool {
    if value.chars().any(char::is_control) {
        return false;
    }
    match url::Url::parse(value.trim()) {
        Ok(url) => {
            matches!(url.scheme(), "http" | "https")
                || (!image && matches!(url.scheme(), "mailto" | "tel"))
        }
        Err(url::ParseError::RelativeUrlWithoutBase) => true,
        Err(_) => false,
    }
}

pub fn safe_css(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value
        .chars()
        .any(|c| c.is_control() || matches!(c, ';' | '{' | '}' | '\\' | '<' | '>'))
        && !corrupted_style(value)
        && !["url(", "expression", "/*", "@"]
            .iter()
            .any(|part| lower.contains(part))
}

pub fn corrupted_style(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("undefinedundefined")
        || lower.split(',').any(|part| {
            let part = part.trim().trim_matches(['\'', '"']);
            part.len() > 18 && part.starts_with("undefined") && part.ends_with("undefined")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_executable_urls_and_css() {
        for value in [
            "javascript:alert(1)",
            "data:text/html,x",
            "java\nscript:x",
            "file:///etc/passwd",
        ] {
            assert!(!safe_url(value, false));
        }
        assert!(safe_url("attachments/a.pdf", false));
        assert!(safe_url("https://example.org/a", true));
        for value in ["red;background:url(x)", "expression(x)", "u\\72l(x)"] {
            assert!(!safe_css(value));
        }
    }
}
