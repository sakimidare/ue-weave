//! Pure mappings from UEditor's filterword.js for surviving Office declarations.
pub fn paragraph_structure(
    element: scraper::ElementRef,
    source: &str,
    disabled: bool,
) -> Option<crate::model::Evidence> {
    let class = element.value().attr("class").unwrap_or("");
    let list = class
        .split_whitespace()
        .any(|name| name.eq_ignore_ascii_case("MsoListParagraph"))
        || element
            .value()
            .attr("style")
            .is_some_and(|style| crate::css::property(style, "mso-list").is_some());
    let heading = class
        .split_whitespace()
        .any(|name| name.starts_with("MsoHeading"));
    if !list && !heading {
        return None;
    }
    Some(crate::rules::evidence(
        serde_json::json!(if disabled {
            "unknown"
        } else if list {
            "likely-list-paragraph"
        } else {
            "likely-heading"
        }),
        if disabled { "unknown" } else { "medium" },
        if disabled {
            "推断已关闭；原始 Word 结构标记保留在节点快照中"
        } else if list {
            "检测到 MsoListParagraph 或 mso-list；记录列表标记和段落顺序，分组与层级记为 unknown"
        } else {
            "检测到 MsoHeading；filterWord 将此类内容转为加粗段落，原始标题层级记为 unknown"
        },
        source,
    ))
}
pub fn vml_dimension(element: scraper::ElementRef, property: &str) -> Option<String> {
    if element.value().name() != "v:imagedata" {
        return None;
    }
    let shape = element
        .ancestors()
        .filter_map(scraper::ElementRef::wrap)
        .find(|node| node.value().name() == "v:shape")?;
    let value = crate::rules::style(shape, property)?;
    let value = value.trim();
    let (number, factor) = if let Some(number) = value.strip_suffix("pt") {
        (number, 96.0 / 72.0)
    } else {
        (value.strip_suffix("px")?, 1.0)
    };
    let pixels = number.trim().parse::<f64>().ok()? * factor;
    (pixels.is_finite() && pixels >= 0.0 && pixels <= u32::MAX as f64)
        .then(|| format!("{}", pixels.round() as u32))
}

pub fn declaration(name: &str, value: &str) -> Option<(&'static str, String)> {
    let value = value.trim();
    let property = match name.trim().to_ascii_lowercase().as_str() {
        "mso-margin-alt" => "margin",
        "mso-margin-top-alt" => "margin-top",
        "mso-margin-right-alt" => "margin-right",
        "mso-margin-bottom-alt" => "margin-bottom",
        "mso-margin-left-alt" => "margin-left",
        "mso-padding-alt" => "padding",
        "mso-padding-top-alt" => "padding-top",
        "mso-padding-right-alt" => "padding-right",
        "mso-padding-bottom-alt" => "padding-bottom",
        "mso-padding-left-alt" => "padding-left",
        "mso-width" => "width",
        "mso-height" => "height",
        "horiz-align" => "text-align",
        "vert-align" | "mso-vertical-align-alt" => "vertical-align",
        "font-color" | "mso-foreground" => "color",
        "mso-background" | "mso-highlight" => "background-color",
        "text-line-through"
            if value.eq_ignore_ascii_case("single") || value.eq_ignore_ascii_case("double") =>
        {
            return Some(("text-decoration", "line-through".into()))
        }
        _ => return None,
    };
    Some((property, value.to_string()))
}
