//! Conservative source-span recovery, verified against the selected parsed DOM.
use scraper::{ElementRef, Html, Selector};

pub fn extract(source: &str, root: ElementRef) -> Option<String> {
    let target = root.value().name();
    let selector = Selector::parse(target).ok()?;
    let expected = root.html();
    let lower = source.to_ascii_lowercase();
    let mut cursor = 0;
    let mut starts = Vec::new();
    let mut matches = Vec::new();
    while cursor < source.len() {
        let start = cursor + source[cursor..].find('<')?;
        if source[start..].starts_with("<!--") {
            cursor = start + 4 + source[start + 4..].find("-->")? + 3;
            continue;
        }
        let bytes = source.as_bytes();
        let mut end = start + 1;
        let mut quote = None;
        while end < bytes.len() {
            let byte = bytes[end];
            if let Some(q) = quote {
                if byte == q {
                    quote = None;
                }
            } else if matches!(byte, b'\'' | b'"') {
                quote = Some(byte);
            } else if byte == b'>' {
                break;
            }
            end += 1;
        }
        if end == bytes.len() {
            return None;
        }
        let token = source[start + 1..end].trim();
        let closing = token.starts_with('/');
        let name = token
            .trim_start_matches('/')
            .split(|ch: char| ch.is_ascii_whitespace() || ch == '/')
            .next()
            .unwrap_or("");
        cursor = end + 1;
        if name.eq_ignore_ascii_case(target) {
            if closing {
                if let Some(open) = starts.pop() {
                    let candidate = &source[open..cursor];
                    let fragment = Html::parse_fragment(candidate);
                    if fragment
                        .select(&selector)
                        .next()
                        .is_some_and(|el| el.html() == expected)
                    {
                        matches.push(candidate.to_string());
                    }
                }
            } else {
                starts.push(start);
            }
        }
        if !closing
            && [
                "script", "style", "textarea", "title", "xmp", "iframe", "noembed", "noframes",
            ]
            .iter()
            .any(|raw| name.eq_ignore_ascii_case(raw))
        {
            let close = format!("</{}", name.to_ascii_lowercase());
            cursor += lower[cursor..].find(&close)?;
        }
        if !source[cursor..].contains('<') {
            break;
        }
    }
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_entities_quotes_comments_and_nested_elements() {
        let body = "<DIV class='Article_Content' data-x='>'><!--x--><div>A&nbsp;&#160;\u{a0}</div><p>尾</p></DIV>";
        let source =
            format!("<script>const s = '<div>伪标签</div>';</script>{body}<footer>end</footer>");
        let document = Html::parse_document(&source);
        let root = document
            .select(&Selector::parse(".Article_Content").unwrap())
            .next()
            .unwrap();
        assert_eq!(extract(&source, root).as_deref(), Some(body));
    }
    #[test]
    fn ambiguous_identical_bodies_require_fallback() {
        let source = "<div class='Article_Content'>A</div><div class='Article_Content'>A</div>";
        let document = Html::parse_document(source);
        let root = document
            .select(&Selector::parse(".Article_Content").unwrap())
            .next()
            .unwrap();
        assert!(extract(source, root).is_none());
    }
}
