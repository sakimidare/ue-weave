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
    !corrupted_style(value) && css_syntax(value)
}

/// 字体名允许保留编辑器异常包装，仍按 CSS token 检查语法。
pub fn safe_font_family(value: &str) -> bool {
    css_syntax(value)
}

fn css_syntax(value: &str) -> bool {
    if value.trim().is_empty() {
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

pub fn repair_corrupted_font(value: &str) -> String {
    let mut input = cssparser::ParserInput::new(value);
    let mut parser = cssparser::Parser::new(&mut input);
    let mut start = 0;
    let mut result = String::new();
    while !parser.is_exhausted() {
        let before = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            return value.to_owned();
        };
        if matches!(token, cssparser::Token::Comma) {
            result.push_str(&repair_font_name(&value[start..before]));
            result.push(',');
            start = parser.position().byte_index();
        }
    }
    result.push_str(&repair_font_name(&value[start..]));
    result
}

fn repair_font_name(part: &str) -> String {
    let trimmed = part.trim();
    let (base, _) = crate::css::value_priority(trimmed);
    let unquoted = base
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| base.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(base);
    let lower = unquoted.to_ascii_lowercase();
    for marker in ["undefinedundefined", "undefined"] {
        if lower.starts_with(marker) && lower.ends_with(marker) && unquoted.len() > marker.len() * 2
        {
            let cleaned = &unquoted[marker.len()..unquoted.len() - marker.len()];
            let mut quoted = String::new();
            if cleaned.contains(char::is_whitespace) || unquoted != base {
                cssparser::serialize_string(cleaned, &mut quoted).expect("writing to String");
            } else {
                quoted.push_str(cleaned);
            }
            let offset = part.len() - part.trim_start().len();
            return format!(
                "{}{}{}",
                &part[..offset],
                quoted,
                &part[offset + base.len()..]
            );
        }
    }
    part.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn font_repair_preserves_valid_names_and_separators() {
        for value in [
            r#""ACME,  Inc",  serif"#,
            "Microsoft YaHei,serif",
            r"ACME\,Inc, serif",
            "  Arial ,  serif  ",
        ] {
            assert_eq!(repair_corrupted_font(value), value);
        }
        assert_eq!(
            repair_corrupted_font(r#""ACME,  Inc",  undefinedMicrosoft YaHeiundefined,serif"#),
            r#""ACME,  Inc",  "Microsoft YaHei",serif"#
        );
        assert_eq!(
            repair_corrupted_font(r#""undefinedMicrosoft YaHeiundefined", serif"#),
            r#""Microsoft YaHei", serif"#
        );
    }
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

    #[test]
    fn repairs_corrupted_font_wrappers() {
        assert_eq!(
            repair_corrupted_font("微软雅黑, undefinedundefinedmicrosoft yaheiundefinedundefined"),
            "微软雅黑, \"microsoft yahei\""
        );
        assert_eq!(
            repair_corrupted_font("MyUndefinedFont, serif"),
            "MyUndefinedFont, serif"
        );
        assert_eq!(
            repair_corrupted_font("undefinedMicrosoft YaHeiundefined !important"),
            "\"Microsoft YaHei\" !important"
        );
        assert_eq!(
            repair_corrupted_font("Arial, sans-serif"),
            "Arial, sans-serif"
        );
    }
}
