//! UEditor 代码块的语言标记及字面文本。
use scraper::{ElementRef, Node};

pub fn valid_language(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || b"_+-#.".contains(&ch))
}

pub fn language(pre: ElementRef) -> Option<String> {
    std::iter::once(pre)
        .chain(
            pre.children()
                .filter_map(ElementRef::wrap)
                .filter(|el| el.value().name() == "code"),
        )
        .find_map(|el| {
            let class = el.value().attr("class").unwrap_or("");
            let brush = class
                .split(';')
                .find_map(|part| part.trim().strip_prefix("brush:"));
            let language = class
                .split_whitespace()
                .find_map(|part| part.strip_prefix("language-"));
            brush
                .or(language)
                .or_else(|| el.value().attr("data-lang"))
                .map(str::trim)
                .filter(|value| valid_language(value))
                .map(str::to_owned)
        })
}

pub fn text(pre: ElementRef) -> String {
    let ueditor = pre.value().attr("class").is_some_and(|class| {
        class
            .split(';')
            .any(|part| part.trim().starts_with("brush:"))
    });
    let mut result = String::new();
    for node in pre.descendants() {
        match node.value() {
            Node::Text(text) => result.push_str(text),
            Node::Element(el) if el.name() == "br" => result.push('\n'),
            _ => {}
        }
    }
    if ueditor {
        result.replace('\u{a0}', " ")
    } else {
        result
    }
}
