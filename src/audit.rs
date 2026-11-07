//! 从原始 DOM 生成诊断，单独检查正文块提取过程涉及的属性。
use crate::diagnostic::{Details, Diagnostic, NoteKind, WarningKind};
use crate::model::Ir;
use serde_json::json;

/// 列出已有正文结构或文字格式映射的 HTML 标签。
fn mapped_element(tag: &str) -> bool {
    matches!(
        crate::tag_registry::handling(tag),
        crate::tag_registry::Handling::Block
            | crate::tag_registry::Handling::Inline
            | crate::tag_registry::Handling::Container
    )
}

pub fn append_diagnostics(ir: &mut Ir) {
    let mut literal_nodes = std::collections::HashSet::new();
    let mut converted_subtrees = std::collections::HashSet::new();
    let mut ueditor_code = std::collections::HashSet::new();
    let attachment_nodes: std::collections::HashSet<&str> = ir
        .attachments
        .iter()
        .filter_map(|attachment| attachment["source_nodes"].as_array())
        .flatten()
        .filter_map(|id| id.as_str())
        .collect();
    for node in &ir.original_nodes {
        let id = &node["id"];
        let tag = node["tag"].as_str().unwrap_or("");
        if node["parent"]
            .as_str()
            .is_some_and(|parent| converted_subtrees.contains(parent))
        {
            converted_subtrees.insert(id.as_str().unwrap_or("").to_owned());
            continue;
        }
        let bookmark = tag == "span"
            && node["attrs"]["id"]
                .as_str()
                .is_some_and(|id| id.to_ascii_lowercase().starts_with("_baidu_bookmark_"));
        if bookmark || matches!(tag, "svg" | "math" | "iframe") {
            converted_subtrees.insert(id.as_str().unwrap_or("").to_owned());
        }
        if bookmark {
            ir.warnings.push(Diagnostic::new(
                NoteKind::EditorBookmark,
                Details {
                    original: node["attrs"].clone(),
                    removed: Some(true),
                    source_nodes: vec![id.as_str().unwrap_or("").into()],
                    ..Default::default()
                },
            ));
            continue;
        }
        let ue_code = (tag == "pre"
            && node["attrs"]["class"].as_str().is_some_and(|class| {
                class
                    .split(';')
                    .any(|part| part.trim().starts_with("brush:"))
            }))
            || node["parent"]
                .as_str()
                .is_some_and(|parent| ueditor_code.contains(parent));
        if ue_code {
            ueditor_code.insert(id.as_str().unwrap_or("").to_owned());
        }
        let literal = tag == "pre"
            || crate::model::InlineKind::from_tag(tag).is_some()
            || node["parent"]
                .as_str()
                .is_some_and(|parent| literal_nodes.contains(parent));
        if literal {
            literal_nodes.insert(id.as_str().unwrap_or("").to_owned());
        }
        let attachment_node = attachment_nodes.contains(id.as_str().unwrap_or(""));
        if matches!(tag, "span" | "div")
            && node["children"].as_array().is_some_and(Vec::is_empty)
            && !attachment_node
        {
            ir.warnings.push(Diagnostic::new(
                NoteKind::EmptyElement,
                Details {
                    original: serde_json::json!({"tag":tag,"attrs":node["attrs"]}),
                    removed: Some(true),
                    ir_field: (if tag == "div" { Some("blocks") } else { None }).map(str::to_owned),
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    reason: Some(
                        (if tag == "div" {
                            "空 div 规范化为空段落；原始节点保留"
                        } else {
                            "空 span 不产生文本片段；原始节点保留"
                        })
                        .to_owned(),
                    ),
                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                    ..Default::default()
                },
            ));
        }
        if node["type"] == "text" {
            let text = node["text"].as_str().unwrap_or("");
            if ue_code && text.contains('\u{a0}') {
                ir.warnings.push(Diagnostic::new(
                    NoteKind::CodeWhitespace,
                    Details {
                        original: json!(text),
                        value: json!(text.replace('\u{a0}', " ")),
                        removed: Some(false),
                        ir_field: Some("blocks.text".into()),
                        source_nodes: vec![id.as_str().unwrap_or("").into()],
                        reason: Some("brush 标记代码块中的不换行空格还原为普通空格".into()),
                        ..Default::default()
                    },
                ));
            }
            let cleaned = crate::rules::clean_sentinel(text);
            if cleaned != text && !literal {
                ir.warnings.push(Diagnostic::new(
                    NoteKind::Undefinedundefined,
                    Details {
                        original: serde_json::json!(text),
                        value: serde_json::json!(cleaned),
                        removed: Some(true),
                        ir_field: Some("blocks.text / runs.text".to_owned()),
                        inferred: Some(false),
                        confidence: Some("high".to_owned()),
                        reason: Some(
                            ("移除已知异常字符串 undefinedundefined；完整原文本保存在节点快照中")
                                .to_owned(),
                        ),
                        source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                        ..Default::default()
                    },
                ));
            }
            let spaces = text.chars().filter(|c| *c == ' ').count();
            let nbsp = text.chars().filter(|c| *c == '\u{a0}').count();
            let newlines = text.chars().filter(|c| *c == '\n').count();
            for (character, codepoint) in [('\u{200b}', "U+200B"), ('\u{feff}', "U+FEFF")] {
                let count = text.chars().filter(|c| *c == character).count();
                if count > 0 {
                    let value = serde_json::json!({
                        "codepoint": codepoint, "count": count, "origin": "unknown"
                    });
                    ir.warnings.push(Diagnostic::new(
                        WarningKind::PossibleEditorFillCharacter,
                        Details {
                            original: serde_json::json!(text),
                            value,
                            removed: Some(false),
                            ir_field: Some("runs.text".to_owned()),
                            inferred: Some(false),
                            confidence: Some("unknown".to_owned()),
                            reason: Some("文字中包含零宽字符，保留待核对".to_owned()),
                            source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                            ..Default::default()
                        },
                    ));
                }
            }
            if !literal && (nbsp > 0 || text.starts_with("  ")) {
                ir.warnings.push(Diagnostic::new(
                    NoteKind::Whitespace,
                    Details {
                        original: serde_json::json!(text),
                        value: serde_json::json!({"spaces":spaces,"nbsp":nbsp,"newlines":newlines}),
                        removed: Some(false),
                        ir_field: Some("runs.text".to_owned()),
                        inferred: Some(false),
                        confidence: Some("high".to_owned()),
                        reason: Some(
                            ("统计 HTML 解码后的普通空格、不换行空格和换行数量").to_owned(),
                        ),
                        source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                        ..Default::default()
                    },
                ));
            }
        }
        if crate::tag_registry::handling(tag) == crate::tag_registry::Handling::Excluded {
            ir.warnings.push(Diagnostic::new(
                WarningKind::ExcludedElement,
                Details {
                    original: serde_json::json!(tag),
                    removed: Some(true),
                    ir_field: None,
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    reason: Some(
                        ("正文重建排除可执行或外部嵌入内容；原节点保留在快照中").to_owned(),
                    ),
                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                    ..Default::default()
                },
            ));
        }
        if !tag.is_empty()
            && !mapped_element(tag)
            && crate::tag_registry::handling(tag) != crate::tag_registry::Handling::Excluded
        {
            ir.warnings.push(Diagnostic::new(
                WarningKind::UnsupportedStructure,
                Details {
                    original: serde_json::json!({"tag":tag,"attrs":node["attrs"]}),
                    value: serde_json::json!("partial"),
                    removed: Some(true),
                    ir_field: Some("original_nodes".to_owned()),
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    reason: Some(
                        match tag {
                            "iframe" | "video" | "audio" | "source" => {
                                "媒体地址转为链接，保留备用文字"
                            }
                            "svg" | "math" => "保留图形说明或公式文字，图形布局未转换",
                            _ => "保留文字和图片，容器语义未转换",
                        }
                        .to_owned(),
                    ),
                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                    ..Default::default()
                },
            ));
        }
        if let Some(attrs) = node["attrs"].as_object() {
            if tag == "sdfield" {
                ir.warnings.push(Diagnostic::new(
                    NoteKind::StaticField,
                    Details {
                        original: json!(attrs),
                        source_nodes: vec![id.as_str().unwrap_or_default().into()],
                        reason: Some(
                            "保留页码、日期等字段当前显示的文字，导出后按普通文字编辑".into(),
                        ),
                        ..Default::default()
                    },
                ));
            }
            if ["ol", "ul"].contains(&tag) {
                if let Some(class) = attrs.get("class").and_then(|value| value.as_str()) {
                    for marker in class
                        .split_whitespace()
                        .filter(|class| class.starts_with("custom_"))
                    {
                        if matches!(marker, "custom_dot" | "custom_dash") && tag == "ul" {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::ListStyle,
                                Details {
                                    original: json!(marker),
                                    value: json!(marker.trim_start_matches("custom_")),
                                    ir_field: Some("blocks.list.style".into()),
                                    reason: Some("使用文字圆点或短横线表示列表标记".into()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().into()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        ir.warnings.push(Diagnostic::new(
                            WarningKind::CustomListMarker,
                            Details {
                                original: serde_json::json!(marker),
                                value: serde_json::json!("unknown"),
                                removed: Some(true),
                                ir_field: Some("blocks.list.custom_style".to_owned()),
                                inferred: Some(false),
                                confidence: Some("unknown".to_owned()),
                                reason: Some("自定义编号转为基础列表".to_owned()),
                                source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                ..Default::default()
                            },
                        ));
                    }
                }
                let unsupported =
                    attrs
                        .get("style")
                        .and_then(|v| v.as_str())
                        .is_some_and(|style| {
                            style.split(';').any(|declaration| {
                                declaration.split_once(':').is_some_and(|(key, value)| {
                                    key.trim() == "list-style-type"
                                        && ![
                                            "decimal",
                                            "lower-alpha",
                                            "upper-alpha",
                                            "lower-latin",
                                            "upper-latin",
                                            "lower-roman",
                                            "upper-roman",
                                            "disc",
                                            "circle",
                                            "square",
                                            "none",
                                            "dot",
                                            "dash",
                                        ]
                                        .contains(&value.trim())
                                })
                            })
                        });
                if unsupported {
                    ir.warnings.push(Diagnostic::new(
                        WarningKind::ExportLimitation,
                        Details {
                            format: Some("typst".to_owned()),
                            original: serde_json::json!(attrs),
                            confidence: Some("high".to_owned()),
                            reason: Some(
                                ("自定义列表标记已写入 IR 和 HTML；Typst 映射待实现").to_owned(),
                            ),
                            source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                            ..Default::default()
                        },
                    ));
                }
            }
            for (name, value) in attrs {
                if crate::rules::is_word_or_webplus_attribute(name) {
                    ir.warnings.push(Diagnostic::new(
                        NoteKind::UeOrWordAttribute,
                        Details {
                            attribute: Some((name).to_owned()),
                            original: serde_json::json!(value),
                            removed: Some(true),
                            ir_field: Some(
                                (if matches!(name.as_str(), "sudyfile-attr" | "isupload")
                                    && tag == "a"
                                {
                                    "attachments.attributes"
                                } else {
                                    "original_nodes.attrs"
                                })
                                .to_owned(),
                            ),
                            inferred: Some(false),
                            confidence: Some("high".to_owned()),
                            reason: Some(
                                ("扩展属性保存在来源记录中，重建 HTML 排除此属性；附件链接保留")
                                    .to_owned(),
                            ),
                            source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                            ..Default::default()
                        },
                    ));
                }
                let raw = value.as_str().unwrap_or("");
                let unsafe_url = ["href", "src"].contains(&name.as_str())
                    && !crate::safety::safe_url(raw, name == "src");
                if name.starts_with("on") || unsafe_url {
                    ir.warnings.push(Diagnostic::new(
                        WarningKind::UnsafeAttribute,
                        Details {
                            attribute: Some((name).to_owned()),
                            original: serde_json::json!(value),
                            removed: Some(true),
                            ir_field: None,
                            inferred: Some(false),
                            confidence: Some("high".to_owned()),
                            reason: Some("重建输出过滤事件属性和不安全 URL".to_owned()),
                            source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                            ..Default::default()
                        },
                    ));
                }
                if name == "style" {
                    for declaration in crate::css::declarations(raw)
                        .into_iter()
                        .filter(|s| !s.trim().is_empty())
                    {
                        let normalized = crate::css::without_comments(declaration);
                        if normalized != declaration {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::CssCommentNormalization,
                                Details {
                                    original: serde_json::json!(declaration),
                                    value: serde_json::json!(normalized),
                                    removed: Some(true),
                                    ir_field: Some("original_nodes.attrs.style".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("读取样式时跳过注释".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                        }
                        if normalized.trim().is_empty() {
                            continue;
                        }
                        let Some((property, raw_value)) = normalized.split_once(':') else {
                            ir.warnings.push(Diagnostic::new(
                                WarningKind::MalformedStyle,
                                Details {
                                    original: serde_json::json!(declaration),
                                    removed: Some(true),
                                    ir_field: None,
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some(
                                        ("CSS 声明缺少冒号，已跳过；原值见 original_nodes")
                                            .to_owned(),
                                    ),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        };
                        let (value, important) = crate::css::value_priority(raw_value);
                        if important {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::CssPriority,
                                Details {
                                    original: serde_json::json!(declaration),
                                    value: serde_json::json!(value),
                                    removed: Some(true),
                                    ir_field: Some("blocks / runs".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("按 !important 和声明顺序选择样式".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                        }
                        let property = property.trim().to_ascii_lowercase();
                        if tag == "img" && matches!(property.as_str(), "width" | "height") {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::ImageDimension,
                                Details {
                                    original: serde_json::json!(declaration),
                                    value: serde_json::json!(value),
                                    removed: Some(!crate::safety::safe_css(value)),
                                    ir_field: Some("runs.image_attributes".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("优先采用样式中的图片宽高".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if tag == "table"
                            && crate::style_registry::TABLE_STYLES.contains(&property.as_str())
                        {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::TableStyle,
                                Details {
                                    original: serde_json::json!(declaration),
                                    value: serde_json::json!(value),
                                    removed: Some(!crate::safety::safe_css(value)),
                                    ir_field: Some("blocks.table_styles".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("保留已识别的表格样式".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if matches!(tag, "col" | "colgroup") && property == "width" {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::ColumnWidth,
                                Details {
                                    original: serde_json::json!(declaration),
                                    value: serde_json::json!(value),
                                    removed: Some(!crate::safety::safe_css(value)),
                                    ir_field: Some("blocks.column_groups".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("提取列宽，按输出格式换算".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        let sentinel =
                            property == "font-family" && crate::safety::corrupted_style(value);
                        if sentinel && crate::safety::safe_font_family(value) {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::UndefinedundefinedStyle,
                                Details {
                                    attribute: Some(property.to_owned()),
                                    original: json!(value.trim()),
                                    ir_field: Some("runs.font_family".into()),
                                    reason: Some(
                                        "检测到编辑器字体名包装；是否修复或移除由输出选项决定"
                                            .into(),
                                    ),
                                    source_nodes: vec![id.as_str().unwrap_or_default().into()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if !crate::safety::safe_css(value.trim()) {
                            ir.warnings.push(Diagnostic::new(
                                WarningKind::UnsafeStyle,
                                Details {
                                    attribute: Some((property).to_owned()),
                                    original: serde_json::json!(value.trim()),
                                    removed: Some(true),
                                    ir_field: Some("original_nodes.attrs.style".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some(
                                        ("重建 HTML 过滤此声明值；原始属性仍保存在节点快照中")
                                            .to_owned(),
                                    ),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6"].contains(&tag)
                            && property == "margin"
                        {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::ParagraphMarginShorthand,
                                Details {
                                    original: serde_json::json!(declaration),
                                    removed: Some(true),
                                    ir_field: Some("blocks.spacing".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("将 margin 拆成各方向间距".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6"].contains(&tag)
                            && ["margin-top", "margin-bottom"].contains(&property.as_str())
                        {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::ParagraphSpacing,
                                Details {
                                    original: serde_json::json!(value.trim()),
                                    attribute: Some((property).to_owned()),
                                    removed: Some(!crate::safety::safe_css(value.trim())),
                                    ir_field: Some("blocks.spacing".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some(
                                        ("将 HTML 段落间距声明写入 blocks.spacing").to_owned(),
                                    ),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if ["td", "th"].contains(&tag)
                            && crate::style_registry::CELL_STYLES.contains(&property.as_str())
                        {
                            let safe = crate::safety::safe_css(value.trim());
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::CellStyle,
                                Details {
                                    attribute: Some((property).to_owned()),
                                    original: serde_json::json!(value.trim()),
                                    removed: Some(!safe),
                                    ir_field: Some("rows.styles".to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("保留已识别的单元格样式".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if let Some((target, mapped)) = crate::word::declaration(&property, value) {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::WordStyleMapping,
                                Details {
                                    attribute: Some((property).to_owned()),
                                    original: serde_json::json!(value.trim()),
                                    value: serde_json::json!(mapped),
                                    removed: Some(true),
                                    ir_field: Some((target).to_owned()),
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some("将 Word 样式转换为标准声明".to_owned()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                            continue;
                        }
                        if crate::style_registry::PAGINATION_STYLES.contains(&property.as_str()) {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::PaginationStyle,
                                Details {
                                    attribute: Some(property),
                                    original: json!(value),
                                    removed: Some(true),
                                    source_nodes: vec![id.as_str().unwrap_or_default().into()],
                                    reason: Some("导出使用目标文档的分页规则".into()),
                                    ..Default::default()
                                },
                            ));
                        } else if ["ul", "ol"].contains(&tag) && property == "list-style-type" {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::ListStyle,
                                Details {
                                    original: json!(value),
                                    ir_field: Some("blocks.list.style".into()),
                                    source_nodes: vec![id.as_str().unwrap_or_default().into()],
                                    ..Default::default()
                                },
                            ));
                        } else if let Some(mapping) = crate::style_registry::common(&property) {
                            ir.metadata
                                .entry("style_mappings")
                                .or_insert_with(|| json!({}))[&property] = json!(mapping.ir_field);
                        } else if crate::style_registry::benign(&property)
                            || crate::style_registry::benign_value(&property, value)
                        {
                            ir.warnings.push(Diagnostic::new(
                                NoteKind::IgnoredStyle,
                                Details {
                                    attribute: Some((property).to_owned()),
                                    original: serde_json::json!(value.trim()),
                                    removed: Some(true),
                                    ir_field: None,
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some(
                                        "保留文字内容，字形、间距和换行由输出格式处理".to_owned(),
                                    ),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                        } else {
                            ir.warnings.push(Diagnostic::new(
                                WarningKind::UnmappedStyle,
                                Details {
                                    attribute: Some((property).to_owned()),
                                    original: serde_json::json!(value.trim()),
                                    removed: Some(true),
                                    ir_field: None,
                                    inferred: Some(false),
                                    confidence: Some("high".to_owned()),
                                    reason: Some(
                                        "该声明写入 original_nodes.attrs.style，IR 格式映射待实现"
                                            .to_owned(),
                                    ),
                                    source_nodes: vec![id.as_str().unwrap_or_default().to_owned()],
                                    ..Default::default()
                                },
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn semantic_downgrades_have_source_evidence() {
        let document = scraper::Html::parse_document("<div class='Article_Content'><custom-box><p>正文</p></custom-box><dl><dt>术语</dt><dd>说明</dd></dl>\
            <table><tbody><tr><td>单元格</td></tr></tbody></table></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        {
            let tag = "custom-box";
            let warning = ir
                .warnings
                .iter()
                .find(|w| w["kind"] == "unsupported-structure" && w["original"]["tag"] == tag)
                .unwrap();
            assert_eq!(warning["inferred"], false);
            let id = &warning["source_nodes"][0];
            assert!(ir
                .original_nodes
                .iter()
                .any(|node| node["id"] == *id && node["tag"] == tag));
        }
        let output = crate::renderer::render(&ir);
        for text in ["正文", "术语", "说明", "单元格"] {
            assert!(output.contains(text));
        }
    }
    #[test]
    fn ambiguous_fill_characters_are_preserved_and_reported() {
        let document = scraper::Html::parse_document(
            "<div class='Article_Content'><p>甲\u{200b}乙\u{feff}丙</p></div>",
        );
        for disabled in [false, true] {
            let ir = crate::parser::parse(&document, ".Article_Content", disabled, None).unwrap();
            let warnings: Vec<_> = ir
                .warnings
                .iter()
                .filter(|w| w["kind"] == "possible-editor-fill-character")
                .collect();
            assert_eq!(warnings.len(), 2);
            for warning in warnings {
                assert_eq!(warning["removed"], false);
                assert_eq!(warning["value"]["origin"], "unknown");
                assert!(!warning["source_nodes"].as_array().unwrap().is_empty());
            }
            assert!(crate::renderer::render(&ir).contains("甲\u{200b}乙\u{feff}丙"));
        }
    }
    #[test]
    fn supported_style_values_do_not_disappear_without_diagnostics() {
        let document = scraper::Html::parse_document(
            "<div class='Article_Content'><p><span style='font-family:undefinedundefinedmicrosoft \
                yaheiundefinedundefined;background-color:url(bad);invalid'>正文</span></p></div>",
        );
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        for kind in [
            "undefinedundefined-style",
            "unsafe-style",
            "malformed-style",
        ] {
            let warning = ir.warnings.iter().find(|w| w["kind"] == kind).unwrap();
            if kind == "undefinedundefined-style" {
                assert!(warning["removed"].is_null());
            } else {
                assert_eq!(warning["removed"], true);
            }
            assert!(warning["original"].is_string());
            assert!(!warning["source_nodes"].as_array().unwrap().is_empty());
        }
        let html = crate::renderer::render(&ir);
        assert!(html.contains("undefinedundefined"));
        assert!(!html.contains("url(bad)"));
        assert!(html.contains("正文"));
    }
    #[test]
    fn explanation_examples_distinguish_media_links_from_removed_elements() {
        for (content, expected, absent) in [
            (
                "<iframe src='https://example.org/player'></iframe>",
                "W0009",
                "W0017",
            ),
            ("<object data='player.swf'></object>", "W0017", "W0009"),
        ] {
            let html = scraper::Html::parse_document(&format!(
                "<div class='Article_Content'>{content}</div>"
            ));
            let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
            assert!(ir.warnings.iter().any(|item| item.kind.code() == expected));
            assert!(!ir.warnings.iter().any(|item| item.kind.code() == absent));
            if expected == "W0009" {
                assert!(
                    crate::renderer::render(&ir).contains("href=\"https://example.org/player\"")
                );
            }
        }
    }

    #[test]
    fn reports_unknowns_and_nested_unsupported_content() {
        let html = scraper::Html::parse_document("<div class='Article_Content'><p>&nbsp;&nbsp;内容<span style='mso-x:1;letter-spacing:2px' \
            onclick='bad()'>文字</span><a href='javascript:x'>链接</a></p><video src='v.mp4'></video></div>");
        let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
        for kind in [
            "whitespace",
            "unmapped-style",
            "unsafe-attribute",
            "unsupported-structure",
        ] {
            assert!(ir.warnings.iter().any(|w| w["kind"] == kind), "{kind}");
        }
        for warning in &ir.warnings {
            for id in warning["source_nodes"].as_array().unwrap() {
                assert!(ir.original_nodes.iter().any(|node| node["id"] == *id));
            }
        }
    }

    #[test]
    fn benign_layout_styles_are_notes_instead_of_warnings() {
        let html = scraper::Html::parse_document(
            "<div class='Article_Content'><p style='text-wrap-mode:wrap;--size:12px;padding:4px;letter-spacing:2px;color:red'>正文</p></div>",
        );
        let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
        for attribute in ["text-wrap-mode", "--size", "padding"] {
            let note = ir
                .warnings
                .iter()
                .find(|w| w["kind"] == "ignored-style" && w["attribute"] == attribute)
                .unwrap_or_else(|| panic!("missing ignored-style for {attribute}"));
            assert_eq!(note["level"], "note");
            assert_eq!(note["removed"], true);
        }
        assert!(ir
            .warnings
            .iter()
            .any(|w| w["kind"] == "unmapped-style" && w["attribute"] == "letter-spacing"));
        assert!(!ir.warnings.iter().any(|w| {
            w["level"] == "warning"
                && ["text-wrap-mode", "--size", "padding"]
                    .contains(&w["attribute"].as_str().unwrap_or(""))
        }));
    }

    #[test]
    fn visual_content_styles_are_reported_before_profile_selection() {
        let html = scraper::Html::parse_document("<div class='Article_Content'><p style='opacity:0;position:absolute;overflow:hidden;transform:scale(0);display:grid;background-image:linear-gradient(red,blue)'>正文</p></div>");
        let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
        for attribute in [
            "opacity",
            "position",
            "overflow",
            "transform",
            "display",
            "background-image",
        ] {
            assert!(
                ir.warnings
                    .iter()
                    .any(|w| w["attribute"] == attribute && w["level"] == "warning"),
                "{attribute}"
            );
            assert!(
                !ir.warnings
                    .iter()
                    .any(|w| w["attribute"] == attribute && w["kind"] == "ignored-style"),
                "{attribute}"
            );
        }
    }
}
