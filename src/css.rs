//! 解析内联 CSS 声明，原始写法保存在节点记录中。
pub fn declarations(input: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut comment_end = 0;
    for (index, ch) in input.char_indices() {
        if index < comment_end {
            continue;
        }
        if quote.is_none() && !escaped && input[index..].starts_with("/*") {
            comment_end = input[index + 2..]
                .find("*/")
                .map(|end| index + end + 4)
                .unwrap_or(input.len());
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                result.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(&input[start..]);
    result
}

pub fn without_comments(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            output.push(ch);
            if let Some(next) = chars.next() {
                output.push(next);
            }
        } else if let Some(q) = quote {
            output.push(ch);
            if ch == q {
                quote = None;
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(next) = chars.next() {
                if next == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            output.push(' ');
        } else {
            if matches!(ch, '\'' | '"') {
                quote = Some(ch);
            }
            output.push(ch);
        }
    }
    output
}

pub fn value_priority(value: &str) -> (&str, bool) {
    if let Some((value, suffix)) = value.rsplit_once('!') {
        if suffix.trim().eq_ignore_ascii_case("important") {
            return (value.trim(), true);
        }
    }
    (value.trim(), false)
}

pub fn property(style: &str, key: &str) -> Option<String> {
    let mut explicit = None;
    let mut alias = None;
    for declaration in declarations(style) {
        let declaration = without_comments(declaration);
        let Some((name, raw)) = declaration.split_once(':') else {
            continue;
        };
        let (value, important) = value_priority(raw);
        if value.is_empty() {
            continue;
        }
        let target = if name.trim().eq_ignore_ascii_case(key) {
            Some((&mut explicit, value.to_string()))
        } else if let Some(value) = box_shorthand(name.trim(), value, key) {
            Some((&mut explicit, value))
        } else {
            crate::word::declaration(name.trim(), value)
                .and_then(|(property, value)| {
                    if property == key {
                        Some(value)
                    } else {
                        box_shorthand(property, &value, key)
                    }
                })
                .map(|value| (&mut alias, value))
        };
        if let Some((slot, value)) = target {
            if slot
                .as_ref()
                .is_none_or(|(_, previous)| !previous || important)
            {
                *slot = Some((value, important));
            }
        }
    }
    explicit.or(alias).map(|(value, _)| value)
}

fn box_shorthand(name: &str, value: &str, key: &str) -> Option<String> {
    let name = name.to_ascii_lowercase();
    if name != "margin" && name != "padding" {
        return None;
    }
    let side = key.strip_prefix(&format!("{name}-"))?;
    let mut depth = 0usize;
    let values: Vec<_> = value
        .split(|ch: char| {
            match ch {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            ch.is_ascii_whitespace() && depth == 0
        })
        .filter(|v| !v.is_empty())
        .collect();
    if values.is_empty() || values.len() > 4 {
        return None;
    }
    let index = match side {
        "top" => 0,
        "right" => {
            if values.len() > 1 {
                1
            } else {
                0
            }
        }
        "bottom" => {
            if values.len() > 2 {
                2
            } else {
                0
            }
        }
        "left" => match values.len() {
            4 => 3,
            2 | 3 => 1,
            _ => 0,
        },
        _ => return None,
    };
    Some(values[index].to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn box_shorthand_obeys_order_and_priority() {
        for (value, bottom) in [
            ("1px", "1px"),
            ("1px 2px", "1px"),
            ("1px 2px 3px", "3px"),
            ("1px 2px 3px 4px", "3px"),
        ] {
            assert_eq!(
                super::property(&format!("margin:{value}"), "margin-bottom").as_deref(),
                Some(bottom)
            );
        }
        assert_eq!(
            super::property("margin-top:8px; margin:2px", "margin-top").as_deref(),
            Some("2px")
        );
        assert_eq!(
            super::property("margin:2px!important; margin-top:8px", "margin-top").as_deref(),
            Some("2px")
        );
        assert_eq!(
            super::property("margin:calc(1em + 2px) 0", "margin-bottom").as_deref(),
            Some("calc(1em + 2px)")
        );
    }
    #[test]
    fn comments_do_not_create_declarations_or_modify_strings() {
        let style = "/* ; color:blue */ color: red !/* priority */important; color:green; font-family:'A/*;*/B'";
        assert_eq!(super::declarations(style).len(), 3);
        assert_eq!(super::property(style, "color").as_deref(), Some("red"));
        assert_eq!(
            super::property(style, "font-family").as_deref(),
            Some("'A/*;*/B'")
        );
        assert_eq!(
            super::property("color:red;/* unfinished;color:blue", "color").as_deref(),
            Some("red")
        );
    }
    #[test]
    fn declaration_boundaries_and_priority() {
        assert_eq!(
            super::declarations("font-family:'A; B';color:rgb(1;2;3);x:y").len(),
            3
        );
        assert_eq!(
            super::property("text-align:right ! IMPORTANT;text-align:left", "text-align")
                .as_deref(),
            Some("right")
        );
        assert_eq!(
            super::property("color:red!important;color:blue!important", "color").as_deref(),
            Some("blue")
        );
        assert_eq!(
            super::property("font-family:'A; B'", "font-family").as_deref(),
            Some("'A; B'")
        );
    }
}
