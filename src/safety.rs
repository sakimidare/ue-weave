/// 允许网页、邮件和相对路径链接，过滤脚本协议。
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
    if corrupted_style(value) || value.trim().is_empty() {
        return false;
    }
    let mut input = cssparser::ParserInput::new(value);
    css_tokens(&mut cssparser::Parser::new(&mut input), 0)
}

fn css_tokens(parser: &mut cssparser::Parser<'_, '_>, depth: usize) -> bool {
    use cssparser::Token;
    if depth > 32 {
        return false;
    }
    while !parser.is_exhausted() {
        let Ok(token) = parser.next().cloned() else {
            return false;
        };
        match token {
            Token::Ident(_)
            | Token::QuotedString(_)
            | Token::Hash(_)
            | Token::IDHash(_)
            | Token::Comma => {}
            Token::Number { value, .. } | Token::Dimension { value, .. } if value.is_finite() => {}
            Token::Percentage { unit_value, .. } if unit_value.is_finite() => {}
            Token::Delim('+' | '-' | '*' | '/') => {}
            Token::Function(name)
                if ["rgb", "rgba", "hsl", "hsla", "calc", "min", "max", "clamp"]
                    .contains(&name.to_ascii_lowercase().as_str()) =>
            {
                let nested: Result<(), cssparser::ParseError<'_, ()>> =
                    parser.parse_nested_block(|nested| {
                        if css_tokens(nested, depth + 1) {
                            Ok(())
                        } else {
                            Err(nested.new_custom_error(()))
                        }
                    });
                if nested.is_err() {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
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
        assert!(safe_css("\"url(\""));
        assert!(safe_css("rgb(255, 0, 0)"));
        assert!(!safe_css("u\\72l(\"https://example.org\")"));
    }
}
