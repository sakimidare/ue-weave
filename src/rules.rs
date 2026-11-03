use crate::model::Evidence;
use scraper::{ElementRef, Selector};
use url::Url;

pub fn evidence(value: serde_json::Value, confidence: &str, reason: &str, node: &str) -> Evidence {
    Evidence {
        value,
        confidence: confidence.into(),
        reason: reason.into(),
        source_nodes: vec![node.into()],
    }
}

pub fn text(el: ElementRef) -> String {
    el.text().collect()
}

pub fn raw(el: ElementRef) -> String {
    el.html()
}

pub fn node_name(el: ElementRef) -> String {
    el.value().name().into()
}

pub fn style(el: ElementRef, key: &str) -> Option<String> {
    el.value()
        .attr("style")
        .and_then(|style| crate::css::property(style, key))
}

pub fn has_tag(el: ElementRef, name: &str) -> bool {
    el.value().name() == name
        || el
            .select(&Selector::parse(name).expect("static selector"))
            .next()
            .is_some()
}

pub fn list_style(el: ElementRef) -> Option<String> {
    style(el, "list-style-type").or_else(|| {
        let value = el.value().attr("type")?;
        let style = match (el.value().name(), value) {
            ("ol", "1") => "decimal",
            ("ol", "a") => "lower-alpha",
            ("ol", "A") => "upper-alpha",
            ("ol", "i") => "lower-roman",
            ("ol", "I") => "upper-roman",
            ("ul", value) if value.eq_ignore_ascii_case("disc") => "disc",
            ("ul", value) if value.eq_ignore_ascii_case("circle") => "circle",
            ("ul", value) if value.eq_ignore_ascii_case("square") => "square",
            _ => return None,
        };
        Some(style.into())
    })
}

pub fn custom_list_style(el: ElementRef<'_>) -> Option<&str> {
    el.value()
        .attr("class")?
        .split_whitespace()
        .find_map(|class| {
            let style = class.strip_prefix("custom_")?;
            ["cn", "cn1", "cn2", "num", "num1", "num2", "dash", "dot"]
                .contains(&style)
                .then_some(style)
        })
}

pub fn attachment_icon(el: ElementRef) -> bool {
    el.value()
        .attr("class")
        .unwrap_or("")
        .split_whitespace()
        .any(|class| class == "sudy-attachment-icon")
        || el
            .value()
            .attr("src")
            .unwrap_or("")
            .contains("icon_doc.gif")
}

pub fn clean_sentinel(input: &str) -> String {
    let mut output = input.to_string();
    loop {
        let lower = output.to_ascii_lowercase();
        let Some(index) = lower.find("undefinedundefined") else {
            return output;
        };
        output.replace_range(index..index + "undefinedundefined".len(), "");
    }
}

pub fn is_word_or_webplus_attribute(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("mso") || lower == "sudyfile-attr" || lower == "isupload"
}

pub fn absolute_url(base: Option<&Url>, value: &str) -> String {
    if value.is_empty() {
        return value.into();
    }
    let Some(base) = base else {
        return value.into();
    };
    base.join(value)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| value.into())
}
