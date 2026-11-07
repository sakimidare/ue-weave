//! 解析、审计和 HTML 导出共用的样式属性列表。
pub const PAGINATION_STYLES: &[&str] = &[
    "widows",
    "orphans",
    "break-inside",
    "break-before",
    "break-after",
    "page-break-inside",
    "page-break-before",
    "page-break-after",
];
/// 对正文还原没有影响的视觉或布局声明。
pub const BENIGN_STYLES: &[&str] = &[
    "text-wrap",
    "text-wrap-mode",
    "text-wrap-style",
    "word-break",
    "overflow-wrap",
    "word-wrap",
    "font-kerning",
    "font-optical-sizing",
    "font-feature-settings",
    "font-variation-settings",
    "font-stretch",
    "font-size-adjust",
    "font-synthesis",
    "font-palette",
    "font-language-override",
    "caret-color",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "background-position",
    "background-position-x",
    "background-position-y",
    "background-size",
    "background-repeat",
    "background-clip",
    "background-origin",
    "background-attachment",
    "background-blend-mode",
    "box-shadow",
    "text-shadow",
    "transform-origin",
    "transition",
    "animation",
    "will-change",
    "zoom",
    "cursor",
    "pointer-events",
    "user-select",
    "clear",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
];

pub fn benign(property: &str) -> bool {
    property.starts_with("--")
        || property.starts_with("font-variant")
        || BENIGN_STYLES.contains(&property)
}

/// 同一属性下只影响视觉、不影响内容的取值。
pub fn benign_value(property: &str, value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    match property {
        "white-space" => matches!(value.as_str(), "normal" | "nowrap"),
        "display" => matches!(value.as_str(), "block" | "inline" | "inline-block"),
        "opacity" => value == "1",
        "position" => value == "static",
        "float" | "filter" | "backdrop-filter" | "transform" | "background-image" => {
            value == "none"
        }
        "overflow" | "overflow-x" | "overflow-y" => value == "visible",
        _ => false,
    }
}

pub const TABLE_STYLES: &[&str] = &[
    "width",
    "height",
    "border",
    "border-collapse",
    "border-spacing",
    "table-layout",
    "background-color",
    "margin-left",
    "margin-right",
];
pub const CELL_STYLES: &[&str] = &[
    "text-align",
    "vertical-align",
    "width",
    "height",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-color",
    "border-width",
    "border-style",
    "background-color",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
];

pub struct Mapping {
    pub property: &'static str,
    pub ir_field: &'static str,
}

pub const COMMON: &[Mapping] = &[
    Mapping {
        property: "font-weight",
        ir_field: "runs.bold",
    },
    Mapping {
        property: "font-style",
        ir_field: "runs.italic",
    },
    Mapping {
        property: "text-decoration",
        ir_field: "runs.underline,strike",
    },
    Mapping {
        property: "font-family",
        ir_field: "runs.font_family",
    },
    Mapping {
        property: "font-size",
        ir_field: "runs.font_size",
    },
    Mapping {
        property: "color",
        ir_field: "runs.color",
    },
    Mapping {
        property: "background-color",
        ir_field: "runs.background_color",
    },
    Mapping {
        property: "background",
        ir_field: "runs.background_color",
    },
    Mapping {
        property: "vertical-align",
        ir_field: "runs.vertical_align",
    },
    Mapping {
        property: "text-indent",
        ir_field: "blocks.indent",
    },
    Mapping {
        property: "text-align",
        ir_field: "blocks.align",
    },
    Mapping {
        property: "line-height",
        ir_field: "blocks.line_height",
    },
];

pub fn common(property: &str) -> Option<&'static Mapping> {
    COMMON.iter().find(|mapping| mapping.property == property)
}

pub fn value(property: &str, value: &str) -> bool {
    (common(property).is_some()
        || TABLE_STYLES.contains(&property)
        || CELL_STYLES.contains(&property)
        || ["margin-top", "margin-bottom", "list-style-type"].contains(&property))
        && if property == "font-family" {
            crate::safety::safe_font_family(value)
        } else {
            crate::safety::safe_css(value)
        }
}

#[cfg(test)]
mod tests {
    #[test]
    fn benign_values_exclude_content_affecting_cases() {
        assert!(super::benign("text-wrap-mode"));
        assert!(super::benign("--size"));
        assert!(super::benign("font-feature-settings"));
        assert!(!super::benign("letter-spacing"));
        assert!(super::benign_value("white-space", "normal"));
        assert!(super::benign_value("white-space", "nowrap"));
        assert!(!super::benign_value("white-space", "pre-wrap"));
        assert!(super::benign_value("display", "block"));
        assert!(!super::benign_value("display", "none"));
    }

    #[test]
    fn registry_has_unique_targets_and_filters_unknown_output_properties() {
        let mut names = std::collections::HashSet::new();
        for mapping in super::COMMON {
            assert!(names.insert(mapping.property));
            assert!(!mapping.ir_field.is_empty());
        }
        assert!(super::value("font-family", "\"url(\""));
        assert!(!super::value("background-image", "url(x)"));
        assert!(!super::value("behavior", "none"));
    }
}
