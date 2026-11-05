use crate::diagnostic::{Details, Diagnostic, NoteKind, WarningKind};
use crate::inline::{image_run, inherited_context, runs, text_run, visit_node, RunContext};
use crate::{
    model::{Block, Ir, Run},
    rules,
};
use scraper::{ElementRef, Html, Node, Selector};
use url::Url;

fn collect_assets(
    root: ElementRef,
    base_url: &Option<Url>,
    ir: &mut Ir,
    snapshot: &crate::snapshot::Snapshot,
) {
    for element in root.select(&Selector::parse(".wp_pdf_player[pdfsrc]").unwrap()) {
        if let Some((href, title)) = crate::attachment::embedded_pdf(element) {
            let absolute = rules::absolute_url(base_url.as_ref(), &href);
            let node = snapshot.element_id(element);
            ir.attachments.push(serde_json::json!({
                "href": absolute,
                "text": title,
                "confidence": "high",
                "reason": "WebPlus PDF 占位元素的 pdfsrc 属性",
                "source_nodes": [node],
            }));
            ir.warnings.push(Diagnostic::new(
                NoteKind::EmbeddedPdf,
                Details {
                    original: serde_json::json!(href),
                    value: serde_json::json!(absolute),
                    removed: Some(false),
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    ir_field: Some("attachments, blocks.runs.link".to_owned()),
                    reason: Some("PDF 阅读器占位元素转换为附件链接".to_owned()),
                    source_nodes: vec![node.to_string()],
                    ..Default::default()
                },
            ));
        }
    }
    for image in root
        .descendants()
        .filter_map(ElementRef::wrap)
        .filter(|element| matches!(element.value().name(), "img" | "v:imagedata"))
    {
        if image.value().name() == "v:imagedata" {
            for property in ["width", "height"] {
                if let Some(value) = crate::word::vml_dimension(image, property) {
                    let original = image
                        .ancestors()
                        .filter_map(ElementRef::wrap)
                        .find(|node| node.value().name() == "v:shape")
                        .and_then(|shape| rules::style(shape, property));
                    ir.warnings.push(Diagnostic::new(
                        NoteKind::VmlDimension,
                        Details {
                            original: serde_json::json!(original),
                            value: serde_json::json!(value),
                            ir_field: Some((format!("image_attributes.{property}")).to_owned()),
                            removed: Some(true),
                            inferred: Some(false),
                            confidence: Some("high".to_owned()),
                            reason: Some("按 pt 与 px 的比例换算图片尺寸".to_owned()),
                            source_nodes: vec![(snapshot.element_id(image)).to_string()],
                            ..Default::default()
                        },
                    ));
                }
            }
            ir.warnings.push(Diagnostic::new(
                NoteKind::VmlImage,
                Details {
                    original: serde_json::json!(image.html()),
                    value: serde_json::json!(image.value().attr("src")),
                    removed: Some(true),
                    ir_field: Some("runs.image".to_owned()),
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    reason: Some("从 VML 标签提取图片地址".to_owned()),
                    source_nodes: vec![(snapshot.element_id(image)).to_string()],
                    ..Default::default()
                },
            ));
        }
        if rules::attachment_icon(image) {
            ir.warnings.push(Diagnostic::new(
                NoteKind::AttachmentIcon,
                Details {
                    original: serde_json::json!(image.html()),
                    action: Some("removed".to_owned()),
                    removed: Some(true),
                    ir_field: None,
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    reason: Some("识别附件装饰图标，输出保留附件链接并排除图标图片".to_owned()),
                    source_nodes: vec![(snapshot.element_id(image)).to_string()],
                    ..Default::default()
                },
            ));
        } else if image
            .value()
            .attr("src")
            .is_some_and(|value| crate::safety::safe_url(value, true))
        {
            ir.images.push(serde_json::json!({
                "src": image.value().attr("src").map(|value|rules::absolute_url(base_url.as_ref(),value)),
                "alt": image.value().attr("alt").unwrap_or(""),
                "source_nodes": [snapshot.element_id(image)],
            }));
        }
    }
    for link in root.select(&Selector::parse("a").unwrap()) {
        if let Some(href) = link.value().attr("href") {
            if !crate::safety::safe_url(href, false) {
                continue;
            }
            if let Some(mut attachment) = crate::attachment::identify(link, href) {
                attachment["href"] =
                    serde_json::json!(rules::absolute_url(base_url.as_ref(), href));
                attachment["text"] = serde_json::json!(rules::text(link));
                attachment["source_nodes"] = serde_json::json!([snapshot.element_id(link)]);
                if attachment["attributes"]["sudyfile_raw"].is_string()
                    && attachment["attributes"]["sudyfile_json"].is_null()
                {
                    ir.warnings.push(Diagnostic::new(
                        WarningKind::AttachmentMetadataUnparsed,
                        Details {
                            original: serde_json::json!(attachment["attributes"]["sudyfile_raw"]),
                            value: serde_json::json!("unknown"),
                            confidence: Some("unknown".to_owned()),
                            reason: Some(
                                ("附件属性解析失败，原值见 attachments.attributes.sudyfile_raw")
                                    .to_owned(),
                            ),
                            source_nodes: vec![(snapshot.element_id(link)).to_string()],
                            ..Default::default()
                        },
                    ));
                }
                ir.attachments.push(attachment);
            }
        }
    }
}

/// 按选择器提取正文，补全相对链接，并收集来源节点与诊断。
/// no_inference 为 true 时仅使用显式格式；函数仅处理已加载的 DOM。
pub fn parse(
    document: &Html,
    selector: &str,
    no_inference: bool,
    source: Option<String>,
) -> crate::error::Result<Ir> {
    let root = document
        .select(&Selector::parse(selector).map_err(|_| {
            crate::error::RecoverError::InvalidSelector {
                selector: selector.into(),
            }
        })?)
        .next()
        .ok_or_else(|| crate::error::RecoverError::ContentNotFound {
            selector: selector.into(),
        })?;
    let mut base_url = source
        .as_deref()
        .and_then(|value| Url::parse(value).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"));
    let snapshot = crate::snapshot::Snapshot::capture(root);
    let mut ir = Ir {
        source,
        ..Default::default()
    };
    if let Some(base) = document
        .select(&Selector::parse("base[href]").unwrap())
        .next()
    {
        let href = base.value().attr("href").unwrap();
        let resolved = base_url
            .as_ref()
            .and_then(|url| url.join(href).ok())
            .or_else(|| Url::parse(href).ok());
        match resolved.filter(|url| matches!(url.scheme(), "http" | "https")) {
            Some(url) => base_url = Some(url),
            None => ir.warnings.push(Diagnostic::new(
                WarningKind::InvalidBaseUrl,
                Details {
                    original: serde_json::json!(href),
                    value: serde_json::json!("ignored"),
                    confidence: Some("high".to_owned()),
                    reason: Some(
                        ("base href 的 HTTP/HTTPS 地址校验失败，使用页面 URL 作为基址").to_owned(),
                    ),
                    ..Default::default()
                },
            )),
        }
    }
    if let Some(base) = &base_url {
        ir.metadata
            .insert("resolved_base_url".into(), serde_json::json!(base.as_str()));
    }
    ir.metadata.extend(crate::site::metadata(document));
    collect_assets(root, &base_url, &mut ir, &snapshot);
    parse_blocks(root, &base_url, no_inference, &snapshot, &mut ir);
    ir.original_nodes = snapshot.nodes;
    crate::audit::append_diagnostics(&mut ir);
    crate::inference::annotate(&mut ir, no_inference);
    crate::integrity::check(root, &mut ir);
    Ok(ir)
}

enum BlockCandidate<'a> {
    Element(ElementRef<'a>),
    Inline(ElementRef<'a>, Vec<Run>),
}

fn is_content_block(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "li"
            | "ul"
            | "ol"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "table"
            | "blockquote"
            | "pre"
            | "hr"
    )
}

fn is_container(tag: &str) -> bool {
    matches!(
        tag,
        "div"
            | "section"
            | "article"
            | "figure"
            | "figcaption"
            | "header"
            | "footer"
            | "main"
            | "aside"
            | "nav"
            | "address"
            | "center"
            | "fieldset"
            | "legend"
            | "dl"
            | "dt"
            | "dd"
            | "details"
            | "summary"
    )
}

fn collect_candidates<'a>(
    root: ElementRef<'a>,
    context: &RunContext,
    snapshot: &crate::snapshot::Snapshot,
    output: &mut Vec<BlockCandidate<'a>>,
) {
    let context = context.for_element(root);
    let mut pending = Vec::new();
    fn flush<'a>(
        root: ElementRef<'a>,
        pending: &mut Vec<Run>,
        output: &mut Vec<BlockCandidate<'a>>,
    ) {
        if pending.iter().any(|run| {
            run.image.is_some()
                || run.line_break
                || run
                    .text
                    .chars()
                    .any(|ch| !matches!(ch, ' ' | '\t' | '\r' | '\n'))
        }) {
            output.push(BlockCandidate::Inline(root, std::mem::take(pending)));
        } else {
            pending.clear();
        }
    }
    for child in root.children() {
        if let Some(element) = ElementRef::wrap(child) {
            let name = element.value().name();
            if matches!(name, "script" | "style" | "iframe" | "object" | "embed") {
                continue;
            }
            if is_content_block(name) || crate::attachment::embedded_pdf(element).is_some() {
                flush(root, &mut pending, output);
                output.push(BlockCandidate::Element(element));
            } else if is_container(name)
                || element
                    .descendants()
                    .skip(1)
                    .filter_map(ElementRef::wrap)
                    .any(|node| {
                        is_content_block(node.value().name()) || is_container(node.value().name())
                    })
            {
                flush(root, &mut pending, output);
                if element.children().next().is_none() && name == "div" {
                    output.push(BlockCandidate::Element(element));
                } else {
                    collect_candidates(element, &context, snapshot, output);
                }
            } else if matches!(name, "img" | "v:imagedata") {
                if let Some(run) = image_run(element, &context, snapshot) {
                    pending.push(run);
                }
            } else {
                visit_node(element, &context, snapshot, &mut pending);
            }
        } else if let Node::Text(text) = child.value() {
            if let Some(mut run) = text_run(text, &context) {
                run.source_nodes = snapshot
                    .id_of(child.id())
                    .map(str::to_owned)
                    .into_iter()
                    .collect();
                pending.push(run);
            }
        }
    }
    flush(root, &mut pending, output);
}

fn parse_blocks(
    root: ElementRef,
    base_url: &Option<Url>,
    no_inference: bool,
    snapshot: &crate::snapshot::Snapshot,
    ir: &mut Ir,
) {
    let mut candidates = Vec::new();
    collect_candidates(
        root,
        &inherited_context(root, base_url),
        snapshot,
        &mut candidates,
    );
    for candidate in candidates {
        let (el, inline) = match candidate {
            BlockCandidate::Element(el) => (el, None),
            BlockCandidate::Inline(el, runs) => (el, Some(runs)),
        };
        let name = if inline.is_some() {
            "p".to_string()
        } else {
            rules::node_name(el)
        };
        let text = inline
            .as_ref()
            .map(|runs| runs.iter().map(|r| r.text.as_str()).collect())
            .unwrap_or_else(|| rules::text(el));
        let text = rules::clean_sentinel(&text);
        let node = snapshot.element_id(el).to_string();
        if matches!(
            name.as_str(),
            "ul" | "ol" | "li" | "blockquote" | "pre" | "hr"
        ) {
            let mut child_ir = Ir::default();
            if !matches!(name.as_str(), "pre" | "hr") {
                parse_blocks(el, base_url, no_inference, snapshot, &mut child_ir);
            }
            ir.warnings.extend(child_ir.warnings);
            ir.blocks.push(Block {
                row_groups: Vec::new(),
                column_groups: Vec::new(),
                table_styles: Default::default(),
                word_structure: None,
                list_value: if name == "li" {
                    el.value().attr("value").and_then(|v| v.trim().parse().ok())
                } else {
                    None
                },
                children: child_ir.blocks,
                spacing: Default::default(),
                caption: None,
                list: matches!(name.as_str(), "ul" | "ol").then(|| crate::model::ListAttributes {
                    custom_style: rules::custom_list_style(el).map(|style| rules::evidence(serde_json::json!(style), "high", "原始类名匹配 UEditor list 插件的 custom_* 标记；具体字形取决于站点的列表图标资源", &node)),
                    ordered: name == "ol",
                    start: el.value().attr("start").and_then(|v| v.parse().ok()),
                    reversed: el.value().attr("reversed").is_some(),
                    style: rules::list_style(el),
                }),
                kind: match name.as_str() {
                    "li" => "list_item",
                    "blockquote" => "quote",
                    "pre" => "code_block",
                    "hr" => "horizontal_rule",
                    _ => "list",
                }
                .into(),
                rows: Vec::new(),
                text,
                runs: Vec::new(),
                indent: rules::evidence(
                    serde_json::json!("unknown"),
                    "unknown",
                    "此结构的缩进未推断",
                    &node,
                ),
                line_height: None,
                align: "unknown".into(),
                raw_html: rules::raw(el),
                source_nodes: vec![node],
                level: None,
                signature: None,
            });
            continue;
        }
        if name == "table" {
            let mut row_groups = Vec::new();
            let rows = el
                .select(&Selector::parse("tr").unwrap())
                .filter(|row| {
                    row.ancestors()
                        .filter_map(ElementRef::wrap)
                        .find(|a| a.value().name() == "table")
                        .is_some_and(|a| a.id() == el.id())
                })
                .map(|row| {
                    row_groups.push(
                        row.parent()
                            .and_then(ElementRef::wrap)
                            .filter(|parent| {
                                matches!(parent.value().name(), "thead" | "tbody" | "tfoot")
                            })
                            .map(|parent| crate::model::RowGroup {
                                tag: parent.value().name().into(),
                                source_node: snapshot.element_id(parent).into(),
                            }),
                    );
                    row.children()
                        .filter_map(ElementRef::wrap)
                        .filter(|cell| matches!(cell.value().name(), "td" | "th"))
                        .map(|cell| {
                            let mut child_ir = Ir::default();
                            if cell
                                .select(
                                    &Selector::parse(
                                        "p,div,table,ul,ol,h1,h2,h3,h4,h5,h6,blockquote,pre,hr",
                                    )
                                    .unwrap(),
                                )
                                .next()
                                .is_some()
                            {
                                parse_blocks(cell, base_url, no_inference, snapshot, &mut child_ir);
                            }
                            ir.warnings.extend(child_ir.warnings);
                            let mut content = Vec::new();
                            if child_ir.blocks.is_empty() {
                                visit_node(
                                    cell,
                                    &inherited_context(cell, base_url),
                                    snapshot,
                                    &mut content,
                                );
                            }
                            crate::model::TableCell {
                                styles: crate::table::cell_styles(cell, snapshot.element_id(cell)),
                                blocks: child_ir.blocks,
                                header: cell.value().name() == "th",
                                colspan: cell
                                    .value()
                                    .attr("colspan")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(1)
                                    .clamp(1, 1000),
                                rowspan: {
                                    let span = crate::table::row_span(cell, row);
                                    if let Some(raw) = cell.value().attr("rowspan") {
                                        if raw.parse::<usize>().ok() != Some(span) {
                                            ir.warnings.push(Diagnostic::new(
                                                NoteKind::RowspanNormalized,
                                                Details {
                                                    original: serde_json::json!(raw),
                                                    value: serde_json::json!(span),
                                                    removed: Some(false),
                                                    ir_field: Some("rows.rowspan".to_owned()),
                                                    inferred: Some(false),
                                                    confidence: Some("high".to_owned()),
                                                    reason: Some(
                                                        ("按当前行组剩余行数解析合并范围")
                                                            .to_owned(),
                                                    ),
                                                    source_nodes: vec![
                                                        (snapshot.element_id(cell)).to_string()
                                                    ],
                                                    ..Default::default()
                                                },
                                            ));
                                        }
                                    }
                                    span
                                },
                                runs: content,
                            }
                        })
                        .collect()
                })
                .collect();
            ir.blocks.push(Block {
                row_groups,
                column_groups: crate::table::column_groups(el, snapshot),
                table_styles: crate::table::table_styles(el, &node),
                word_structure: None,
                list_value: None,
                children: Vec::new(),
                spacing: Default::default(),
                caption: el
                    .children()
                    .filter_map(ElementRef::wrap)
                    .find(|child| child.value().name() == "caption")
                    .map(|caption| runs(caption, base_url, snapshot)),
                list: None,
                line_height: None,
                rows,
                kind: "table".into(),
                text,
                indent: rules::evidence(
                    serde_json::json!("unknown"),
                    "unknown",
                    "表格不推断缩进",
                    &node,
                ),
                align: "unknown".into(),
                runs: vec![],
                raw_html: rules::raw(el),
                source_nodes: vec![node],
                level: None,
                signature: None,
            });
            continue;
        }
        if el
            .value()
            .attr("style")
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains("mso-")
        {
            ir.warnings.push(Diagnostic::new(
                NoteKind::WordStyle,
                Details {
                    original: serde_json::json!(el.value().attr("style")),
                    removed: Some(true),
                    ir_field: Some("original_nodes.attrs.style".to_owned()),
                    inferred: Some(false),
                    confidence: Some("high".to_owned()),
                    reason: Some(
                        ("原始样式包含 Office 声明；支持的声明逐项映射，完整原值保留在节点快照中")
                            .to_owned(),
                    ),
                    source_nodes: vec![node.to_string()],
                    ..Default::default()
                },
            ));
        }
        let indent = if let Some(value) = rules::style(el, "text-indent") {
            rules::evidence(
                serde_json::json!(value),
                "high",
                "原始段落含 text-indent",
                &node,
            )
        } else if !no_inference
            && text
                .chars()
                .take_while(|c| *c == ' ' || *c == '\u{a0}')
                .count()
                >= 2
        {
            rules::evidence(
                serde_json::json!("likely"),
                "medium",
                "根据段首连续空格/NBSP 推断首行缩进",
                &node,
            )
        } else {
            rules::evidence(
                serde_json::json!("none"),
                "high",
                "未发现首行缩进证据",
                &node,
            )
        };
        let align = rules::style(el, "text-align")
            .or_else(|| el.value().attr("align").map(|value| value.to_string()))
            .or_else(|| {
                el.ancestors()
                    .filter_map(ElementRef::wrap)
                    .find_map(|parent| {
                        rules::style(parent, "text-align")
                            .or_else(|| parent.value().attr("align").map(str::to_string))
                    })
            })
            .unwrap_or("unknown".into());
        let native_level = match name.as_str() {
            "h1" => Some(1),
            "h2" => Some(2),
            "h3" => Some(3),
            "h4" => Some(4),
            "h5" => Some(5),
            "h6" => Some(6),
            _ => None,
        };
        let numbered = text.trim().split_once('、').is_some_and(|(prefix, _)| {
            !prefix.is_empty() && prefix.chars().all(|c| "一二三四五六七八九十百".contains(c))
        });
        let kind = if name == "li" {
            "list_item"
        } else if native_level.is_some()
            || (!no_inference
                && numbered
                && text.trim().chars().count() <= 40
                && (rules::has_tag(el, "strong") || rules::has_tag(el, "b")))
        {
            "heading"
        } else {
            "paragraph"
        };
        let level = (kind == "heading").then(|| {
            rules::evidence(
                serde_json::json!(native_level.unwrap_or(2)),
                "high",
                if native_level.is_some() {
                    "原始 HTML 标题标签"
                } else {
                    "中文编号短段落且原始文本加粗"
                },
                &node,
            )
        });
        let signature = None;
        if inline.is_some() {
            ir.warnings.push(Diagnostic::new(
                NoteKind::InlineBlockGroup,
                Details {
                    value: serde_json::json!("paragraph"),
                    confidence: Some("high".to_owned()),
                    reason: Some("将连续行内节点按原顺序组合为段落".to_owned()),
                    source_nodes: vec![node.to_string()],
                    ..Default::default()
                },
            ));
        }
        let mut block_runs = inline.unwrap_or_else(|| runs(el, base_url, snapshot));
        for run in &mut block_runs {
            if let Some(link) = &run.link {
                run.link = Some(rules::absolute_url(base_url.as_ref(), link));
            }
            if let Some(image) = &run.image {
                run.image = Some(rules::absolute_url(base_url.as_ref(), image));
            }
        }
        let line_height = std::iter::once(el)
            .chain(el.ancestors().filter_map(ElementRef::wrap))
            .take_while(|ancestor| snapshot.contains(*ancestor))
            .find_map(|ancestor| {
                let value = rules::style(ancestor, "line-height")?;
                if matches!(value.to_ascii_lowercase().as_str(), "inherit" | "unset") {
                    return None;
                }
                let value = if value.eq_ignore_ascii_case("initial") {
                    "normal".into()
                } else {
                    value
                };
                let inherited = ancestor.id() != el.id();
                let relative = inherited
                    && ["em", "ex", "%", "ch"]
                        .iter()
                        .any(|unit| value.ends_with(unit));
                let evidence = rules::evidence(
                    serde_json::json!(value),
                    if relative { "medium" } else { "high" },
                    if relative {
                        "继承祖先的相对行距声明；按相对单位记录，中等置信度"
                    } else if inherited {
                        "继承正文范围内最近祖先的 line-height 声明"
                    } else {
                        "原始段落含 line-height 声明"
                    },
                    snapshot.element_id(ancestor),
                );
                if relative {
                    ir.warnings.push(Diagnostic::new(
                        NoteKind::RelativeLineHeight,
                        Details {
                            value: serde_json::json!(evidence.value),
                            confidence: Some((evidence.confidence).to_owned()),
                            reason: Some((evidence.reason).to_owned()),
                            source_nodes: evidence.source_nodes.clone(),
                            inferred: Some(true),
                            ir_field: Some("blocks.line_height".to_owned()),
                            ..Default::default()
                        },
                    ));
                }
                Some(evidence)
            });
        let word_structure = crate::word::paragraph_structure(el, &node, no_inference);
        if let Some(evidence) = &word_structure {
            ir.warnings.push(Diagnostic::new(
                NoteKind::WordParagraphStructure,
                Details {
                    value: serde_json::json!(evidence.value),
                    confidence: Some((evidence.confidence).to_owned()),
                    reason: Some((evidence.reason).to_owned()),
                    source_nodes: evidence.source_nodes.clone(),
                    inferred: Some(!no_inference),
                    removed: Some(false),
                    ir_field: Some("blocks.word_structure".to_owned()),
                    ..Default::default()
                },
            ));
        }
        ir.blocks.push(Block {
            row_groups: Vec::new(),
            column_groups: Vec::new(),
            table_styles: Default::default(),
            word_structure,
            list_value: None,
            spacing: ["margin-top", "margin-bottom"]
                .into_iter()
                .filter_map(|property| {
                    rules::style(el, property).map(|value| {
                        (
                            property.to_string(),
                            rules::evidence(
                                serde_json::json!(value),
                                "high",
                                "最终 HTML 的显式段落间距声明",
                                &node,
                            ),
                        )
                    })
                })
                .collect(),
            children: Vec::new(),
            caption: None,
            list: None,
            line_height,
            rows: Vec::new(),
            kind: kind.into(),
            text: block_runs.iter().map(|run| run.text.as_str()).collect(),
            indent,
            align,
            runs: block_runs,
            raw_html: rules::raw(el),
            source_nodes: vec![node],
            level,
            signature,
        });
    }
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
