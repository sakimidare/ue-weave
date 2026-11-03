//! Diagnostics derived from the preserved DOM, independently of block extraction.
use crate::model::Ir;
use serde_json::json;

/// Elements whose structural or inline meaning has an explicit IR mapping.
fn mapped_element(tag: &str) -> bool {
    if matches!(tag, "thead" | "tbody" | "tfoot") {
        return true;
    }
    matches!(
        tag,
        "p" | "div"
            | "span"
            | "strong"
            | "b"
            | "em"
            | "i"
            | "u"
            | "s"
            | "strike"
            | "del"
            | "sup"
            | "sub"
            | "font"
            | "br"
            | "a"
            | "img"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "tr"
            | "td"
            | "th"
            | "caption"
            | "blockquote"
            | "pre"
            | "hr"
    )
}

pub fn append_diagnostics(ir: &mut Ir) {
    for field in [
        "original_word_styles",
        "original_word_fonts",
        "editor_operation_order",
        "filterword_deleted_styles",
        "original_paragraph_spacing",
        "original_page_setup",
    ] {
        ir.warnings.push(json!({"kind":"unrecoverable","field":field,"value":"unknown","confidence":"unknown","reason":"最终 HTML 无法证明原始文档中该属性的值","source_nodes":[]}));
    }
    for node in &ir.original_nodes {
        let id = &node["id"];
        let tag = node["tag"].as_str().unwrap_or("");
        if matches!(tag, "span" | "div") && node["children"].as_array().is_some_and(Vec::is_empty) {
            ir.warnings.push(json!({"kind":"empty-element","original":{"tag":tag,"attrs":node["attrs"]},"removed":true,"ir_field":if tag == "div" {Some("blocks")} else {None},"inferred":false,"confidence":"high","reason":if tag == "div" {"空 div 规范化为空段落；原始节点保留"} else {"空 span 不产生文本片段；原始节点保留"},"source_nodes":[id]}));
        }
        if node["type"] == "text" {
            let text = node["text"].as_str().unwrap_or("");
            let cleaned = crate::rules::clean_sentinel(text);
            if cleaned != text {
                ir.warnings.push(json!({"kind":"undefinedundefined","original":text,"value":cleaned,"removed":true,"ir_field":"blocks.text / runs.text","inferred":false,"confidence":"high","reason":"移除已知异常字符串 undefinedundefined；完整原文本保存在节点快照中","source_nodes":[id]}));
            }
            let spaces = text.chars().filter(|c| *c == ' ').count();
            let nbsp = text.chars().filter(|c| *c == '\u{a0}').count();
            let newlines = text.chars().filter(|c| *c == '\n').count();
            for (character, codepoint) in [('\u{200b}', "U+200B"), ('\u{feff}', "U+FEFF")] {
                let count = text.chars().filter(|c| *c == character).count();
                if count > 0 {
                    ir.warnings.push(json!({"kind":"possible-editor-fill-character","original":text,"value":{"codepoint":codepoint,"count":count,"origin":"unknown"},"removed":false,"ir_field":"runs.text","inferred":false,"confidence":"unknown","reason":"UEditor domUtils.fillChar 使用 U+200B，IE6 使用 U+FEFF；字符也可能属于作者内容，无法确认来源，保留原字符","source_nodes":[id]}));
                }
            }
            if nbsp > 0 || text.starts_with("  ") {
                ir.warnings.push(json!({"kind":"whitespace","original":text,"value":{"spaces":spaces,"nbsp":nbsp,"newlines":newlines},"removed":false,"ir_field":"runs.text","inferred":false,"confidence":"high","reason":"DOM 解码后的空白计数；实体原始写法需要查阅输入快照","source_nodes":[id]}));
            }
        }
        if ["script", "style", "iframe", "object", "embed"].contains(&tag) {
            ir.warnings.push(json!({"kind":"excluded-element","original":tag,"removed":true,"ir_field":null,"inferred":false,"confidence":"high","reason":"正文重建排除可执行或外部嵌入内容；原节点保留在快照中","source_nodes":[id]}));
        }
        if !tag.is_empty()
            && !mapped_element(tag)
            && !["script", "style", "iframe", "object", "embed"].contains(&tag)
        {
            ir.warnings.push(json!({"kind":"unsupported-structure","original":{"tag":tag,"attrs":node["attrs"]},"value":"partial","removed":true,"ir_field":"original_nodes","inferred":false,"confidence":"high","reason":"此标签的完整语义尚未映射到编辑模型；重建时处理可识别子内容，容器、交互行为或专有结构仅保存在原始快照中","source_nodes":[id]}));
        }
        if let Some(attrs) = node["attrs"].as_object() {
            if ["ol", "ul"].contains(&tag) {
                if let Some(class) = attrs.get("class").and_then(|value| value.as_str()) {
                    for marker in class
                        .split_whitespace()
                        .filter(|class| class.starts_with("custom_"))
                    {
                        ir.warnings.push(json!({"kind":"custom-list-marker","original":marker,"value":"unknown","removed":true,"ir_field":"blocks.list.custom_style","inferred":false,"confidence":"unknown","reason":"UEditor 自定义标记依赖外部 GIF 图标和展示样式；保留类型及原始类名，导出使用基础列表，标记外观尚未还原","source_nodes":[id]}));
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
                                        ]
                                        .contains(&value.trim())
                                })
                            })
                        });
                if unsupported {
                    ir.warnings.push(json!({"kind":"export-limitation","format":"typst","original":attrs,"confidence":"high","reason":"此自定义列表标记尚无 Typst 映射；IR 与 HTML 保留原声明","source_nodes":[id]}));
                }
            }
            for (name, value) in attrs {
                if crate::rules::is_word_or_webplus_attribute(name) {
                    ir.warnings.push(json!({"kind":"ue-or-word-attribute","attribute":name,"original":value,"removed":true,"ir_field":if matches!(name.as_str(), "sudyfile-attr" | "isupload") && tag == "a" {"attachments.attributes"} else {"original_nodes.attrs"},"inferred":false,"confidence":"high","reason":"扩展属性保存在来源记录中，重建 HTML 排除此属性；附件链接保留","source_nodes":[id]}));
                }
                let raw = value.as_str().unwrap_or("");
                let unsafe_url = ["href", "src"].contains(&name.as_str())
                    && !crate::safety::safe_url(raw, name == "src");
                if name.starts_with("on") || unsafe_url {
                    ir.warnings.push(json!({"kind":"unsafe-attribute","attribute":name,"original":value,"removed":true,"ir_field":null,"inferred":false,"confidence":"high","reason":"重建输出过滤事件属性和不安全 URL","source_nodes":[id]}));
                }
                if name == "style" {
                    for declaration in crate::css::declarations(raw)
                        .into_iter()
                        .filter(|s| !s.trim().is_empty())
                    {
                        let normalized = crate::css::without_comments(declaration);
                        if normalized != declaration {
                            ir.warnings.push(json!({"kind":"css-comment-normalization","original":declaration,"value":normalized,"removed":true,"ir_field":"original_nodes.attrs.style","inferred":false,"confidence":"high","reason":"解析声明时忽略字符串外的 CSS 注释，完整原声明保留在节点快照中","source_nodes":[id]}));
                        }
                        if normalized.trim().is_empty() {
                            continue;
                        }
                        let Some((property, raw_value)) = normalized.split_once(':') else {
                            ir.warnings.push(json!({"kind":"malformed-style","original":declaration,"removed":true,"ir_field":null,"inferred":false,"confidence":"high","reason":"声明缺少属性和值的分隔符，保留原始节点供核对","source_nodes":[id]}));
                            continue;
                        };
                        let (value, important) = crate::css::value_priority(raw_value);
                        if important {
                            ir.warnings.push(json!({"kind":"css-priority","original":declaration,"value":value,"removed":true,"ir_field":"blocks / runs","inferred":false,"confidence":"high","reason":"按内联声明优先级选择有效值；编辑模型保存选择后的值，原始优先级标记保留在快照中","source_nodes":[id]}));
                        }
                        let property = property.trim().to_ascii_lowercase();
                        if tag == "img" && matches!(property.as_str(), "width" | "height") {
                            ir.warnings.push(json!({"kind":"image-dimension","original":declaration,"value":value,"removed":!crate::safety::safe_css(value),"ir_field":"runs.image_attributes","inferred":false,"confidence":"high","reason":"图片 CSS 尺寸优先于 HTML 尺寸属性；保留原单位，导出时转换支持的长度","source_nodes":[id]}));
                            continue;
                        }
                        if tag == "table" && crate::table::TABLE_STYLES.contains(&property.as_str())
                        {
                            ir.warnings.push(json!({"kind":"table-style","original":declaration,"value":value,"removed":!crate::safety::safe_css(value),"ir_field":"blocks.table_styles","inferred":false,"confidence":"high","reason":"保留显式表格声明；HTML 输出进行安全过滤，Typst 表格整体样式映射仍有限","source_nodes":[id]}));
                            continue;
                        }
                        if matches!(tag, "col" | "colgroup") && property == "width" {
                            ir.warnings.push(json!({"kind":"column-width","original":declaration,"value":value,"removed":!crate::safety::safe_css(value),"ir_field":"blocks.column_groups","inferred":false,"confidence":"high","reason":"列宽声明保留在 IR 和安全 HTML 中；Typst 映射固定长度和 0–100% 百分比，复杂表达式保留原始声明","source_nodes":[id]}));
                            continue;
                        }
                        if !crate::safety::safe_css(value.trim()) {
                            let sentinel = crate::safety::corrupted_style(value);
                            ir.warnings.push(json!({"kind":if sentinel {"undefinedundefined-style"} else {"unsafe-style"},"attribute":property,"original":value.trim(),"removed":true,"ir_field":"original_nodes.attrs.style","inferred":false,"confidence":"high","reason":"重建 HTML 过滤此声明值；原始属性仍保存在节点快照中","source_nodes":[id]}));
                            continue;
                        }
                        if ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6"].contains(&tag)
                            && property == "margin"
                        {
                            ir.warnings.push(json!({"kind":"paragraph-margin-shorthand","original":declaration,"removed":true,"ir_field":"blocks.spacing","inferred":false,"confidence":"high","reason":"解析简写并按优先级保存段前、段后间距；左右外边距尚未映射，完整声明保存在快照中","source_nodes":[id]}));
                            continue;
                        }
                        if ["p", "div", "h1", "h2", "h3", "h4", "h5", "h6"].contains(&tag)
                            && ["margin-top", "margin-bottom"].contains(&property.as_str())
                        {
                            ir.warnings.push(json!({"kind":"paragraph-spacing","original":value.trim(),"attribute":property,"removed":!crate::safety::safe_css(value.trim()),"ir_field":"blocks.spacing","inferred":false,"confidence":"high","reason":"保留最终 HTML 声明；原始 Word 段落间距仍无法确定","source_nodes":[id]}));
                            continue;
                        }
                        if ["td", "th"].contains(&tag)
                            && crate::table::CELL_STYLES.contains(&property.as_str())
                        {
                            let safe = crate::safety::safe_css(value.trim());
                            ir.warnings.push(json!({"kind":"cell-style","attribute":property,"original":value.trim(),"removed":!safe,"ir_field":"rows.styles","inferred":false,"confidence":"high","reason":"单元格声明保留在 IR；HTML 输出进行安全过滤，Typst 单元格格式映射仍有限","source_nodes":[id]}));
                            continue;
                        }
                        if let Some((target, mapped)) = crate::word::declaration(&property, value) {
                            ir.warnings.push(json!({"kind":"word-style-mapping","attribute":property,"original":value.trim(),"value":mapped,"removed":true,"ir_field":target,"inferred":false,"confidence":"high","reason":"依据 UEditor filterword.js 的声明映射；现存标准 CSS 声明优先","source_nodes":[id]}));
                            continue;
                        }
                        if ![
                            "font-weight",
                            "font-style",
                            "text-decoration",
                            "font-family",
                            "font-size",
                            "color",
                            "background-color",
                            "background",
                            "vertical-align",
                            "text-indent",
                            "text-align",
                            "line-height",
                        ]
                        .contains(&property.as_str())
                        {
                            ir.warnings.push(json!({"kind":"unmapped-style","attribute":property,"original":value.trim(),"removed":true,"ir_field":null,"inferred":false,"confidence":"high","reason":"该声明仅保存在原始节点中，尚未映射到编辑模型","source_nodes":[id]}));
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
        let document = scraper::Html::parse_document("<div class='Article_Content'><custom-box><p>正文</p></custom-box><dl><dt>术语</dt><dd>说明</dd></dl><table><tbody><tr><td>单元格</td></tr></tbody></table></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        for tag in ["custom-box", "dl", "dt", "dd"] {
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
        let document = scraper::Html::parse_document("<div class='Article_Content'><p><span style='font-family:undefinedundefinedmicrosoft yaheiundefinedundefined;background-color:url(bad);invalid'>正文</span></p></div>");
        let ir = crate::parser::parse(&document, ".Article_Content", false, None).unwrap();
        for kind in [
            "undefinedundefined-style",
            "unsafe-style",
            "malformed-style",
        ] {
            let warning = ir.warnings.iter().find(|w| w["kind"] == kind).unwrap();
            assert_eq!(warning["removed"], true);
            assert!(warning["original"].is_string());
            assert!(!warning["source_nodes"].as_array().unwrap().is_empty());
        }
        let html = crate::renderer::render(&ir);
        assert!(!html.contains("undefinedundefined"));
        assert!(!html.contains("url(bad)"));
        assert!(html.contains("正文"));
    }
    #[test]
    fn reports_unknowns_and_nested_unsupported_content() {
        let html = scraper::Html::parse_document("<div class='Article_Content'><p>&nbsp;&nbsp;内容<span style='mso-x:1;letter-spacing:2px' onclick='bad()'>文字</span><a href='javascript:x'>链接</a></p><video src='v.mp4'></video></div>");
        let ir = crate::parser::parse(&html, ".Article_Content", false, None).unwrap();
        for kind in [
            "unrecoverable",
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
}
