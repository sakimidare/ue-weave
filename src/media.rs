//! 嵌入媒体以可编辑链接保留，图形和公式保留文字替代内容。
use scraper::ElementRef;

pub fn bookmark(el: ElementRef) -> bool {
    el.value().name() == "span"
        && el
            .value()
            .attr("id")
            .is_some_and(|id| id.to_ascii_lowercase().starts_with("_baidu_bookmark_"))
}

pub fn special(tag: &str) -> bool {
    matches!(tag, "iframe" | "video" | "audio" | "svg" | "math")
}

pub fn hidden_asset(el: ElementRef) -> bool {
    std::iter::once(el)
        .chain(el.ancestors().filter_map(ElementRef::wrap))
        .any(|el| {
            bookmark(el)
                || matches!(
                    el.value().name(),
                    "script" | "style" | "object" | "embed" | "iframe" | "svg" | "math"
                )
        })
}

pub fn links(el: ElementRef) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let label = match el.value().name() {
        "video" => "视频",
        "audio" => "音频",
        _ => "嵌入页面",
    };
    for node in std::iter::once(el).chain(
        el.children()
            .filter_map(ElementRef::wrap)
            .filter(|child| child.value().name() == "source"),
    ) {
        if let Some(src) = node
            .value()
            .attr("src")
            .filter(|src| crate::safety::safe_url(src, false))
        {
            if !result.iter().any(|(url, _)| url == src) {
                result.push((
                    src.to_owned(),
                    el.value().attr("title").unwrap_or(label).to_owned(),
                ));
            }
        }
    }
    result
}

pub fn alternative(el: ElementRef) -> String {
    let selected: Vec<_> = el
        .descendants()
        .filter_map(ElementRef::wrap)
        .filter(|child| {
            if el.value().name() == "svg" {
                matches!(child.value().name(), "title" | "desc" | "text")
            } else {
                child.value().name() == "annotation"
            }
        })
        .collect();
    if selected.is_empty() && el.value().name() == "math" {
        return crate::rules::text(el);
    }
    selected
        .iter()
        .map(|child| crate::rules::text(*child))
        .collect::<Vec<_>>()
        .join(" ")
}
