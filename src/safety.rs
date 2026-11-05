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
    value
        .split(',')
        .map(str::trim)
        .map(|part| {
            let (base, important) = crate::css::value_priority(part);
            let lower = base.to_ascii_lowercase();
            let mut cleaned = base.to_string();
            for marker in ["undefinedundefined", "undefined"] {
                if lower.starts_with(marker)
                    && lower.ends_with(marker)
                    && base.len() > marker.len() * 2
                {
                    cleaned = base[marker.len()..base.len() - marker.len()].to_string();
                    break;
                }
            }
            if cleaned.contains(char::is_whitespace)
                && !cleaned.starts_with(['\'', '"'])
                && !cleaned.ends_with(['\'', '"'])
            {
                cleaned = format!("\"{cleaned}\"");
            }
            if important {
                format!("{cleaned} !important")
            } else {
                cleaned
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
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
