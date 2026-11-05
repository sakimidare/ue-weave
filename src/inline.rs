use crate::{model::Run, rules, snapshot::Snapshot};
use scraper::{ElementRef, Node};
use url::Url;

#[derive(Clone, Default)]
pub(crate) struct RunContext {
    inline_kind: Option<crate::model::InlineKind>,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    font_family: Option<String>,
    font_size: Option<String>,
    color: Option<String>,
    background_color: Option<String>,
    vertical_align: Option<String>,
    link: Option<String>,
    base_url: Option<Url>,
}

impl RunContext {
    pub(crate) fn for_element(&self, element: ElementRef) -> Self {
        let name = element.value().name();
        Self {
            inline_kind: crate::model::InlineKind::from_tag(name).or(self.inline_kind),
            bold: match rules::style(element, "font-weight")
                .map(|v| v.to_ascii_lowercase())
                .as_deref()
            {
                Some("normal" | "400" | "initial") => false,
                Some("inherit" | "unset") => self.bold,
                Some("bold" | "bolder" | "600" | "700" | "800" | "900") => true,
                _ => self.bold || matches!(name, "strong" | "b"),
            },
            italic: match rules::style(element, "font-style")
                .map(|v| v.to_ascii_lowercase())
                .as_deref()
            {
                Some("normal" | "initial") => false,
                Some("inherit" | "unset") => self.italic,
                Some("italic" | "oblique") => true,
                _ => self.italic || matches!(name, "em" | "i"),
            },
            underline: self.underline
                || name == "u"
                || rules::style(element, "text-decoration")
                    .is_some_and(|v| v.split_whitespace().any(|v| v == "underline")),
            strike: self.strike
                || matches!(name, "s" | "strike" | "del")
                || rules::style(element, "text-decoration")
                    .is_some_and(|v| v.split_whitespace().any(|v| v == "line-through")),
            font_family: inherited_value(
                rules::style(element, "font-family")
                    .or_else(|| element.value().attr("face").map(str::to_string)),
                &self.font_family,
            ),
            font_size: inherited_value(
                rules::style(element, "font-size").or_else(|| {
                    if name == "font" {
                        element
                            .value()
                            .attr("size")
                            .and_then(|size| match size {
                                "1" => Some("10px"),
                                "2" => Some("12px"),
                                "3" => Some("16px"),
                                "4" => Some("18px"),
                                "5" => Some("24px"),
                                "6" => Some("32px"),
                                "7" => Some("48px"),
                                _ => None,
                            })
                            .map(str::to_string)
                    } else {
                        None
                    }
                }),
                &self.font_size,
            ),
            color: inherited_value(
                rules::style(element, "color")
                    .or_else(|| element.value().attr("color").map(str::to_string)),
                &self.color,
            ),
            background_color: rules::style(element, "background-color")
                .or_else(|| rules::style(element, "background"))
                .or(self.background_color.clone()),
            vertical_align: rules::style(element, "vertical-align")
                .or_else(|| match name {
                    "sup" => Some("super".into()),
                    "sub" => Some("sub".into()),
                    _ => None,
                })
                .or(self.vertical_align.clone()),
            link: if name == "a" {
                element
                    .value()
                    .attr("href")
                    .filter(|value| crate::safety::safe_url(value, false))
                    .map(|value| rules::absolute_url(self.base_url.as_ref(), value))
            } else {
                self.link.clone()
            },
            base_url: self.base_url.clone(),
        }
    }
}

fn inherited_value(value: Option<String>, parent: &Option<String>) -> Option<String> {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("inherit" | "unset") => parent.clone(),
        Some("initial") => None,
        _ => value,
    }
}

pub(crate) fn text_run(text: &str, context: &RunContext) -> Option<Run> {
    let text = if context.inline_kind.is_some() {
        text.to_owned()
    } else {
        rules::clean_sentinel(text)
    };
    (!text.is_empty()).then(|| Run {
        inline_kind: context.inline_kind,
        source_nodes: Vec::new(),
        line_break: false,
        text,
        bold: context.bold,
        italic: context.italic,
        underline: context.underline,
        strike: context.strike,
        font_family: context.font_family.clone(),
        font_size: context.font_size.clone(),
        color: context.color.clone(),
        background_color: context.background_color.clone(),
        vertical_align: context.vertical_align.clone(),
        link: context.link.clone(),
        image: None,
        image_attributes: None,
        source: "original".into(),
    })
}

pub(crate) fn image_run(
    element: ElementRef,
    context: &RunContext,
    snapshot: &Snapshot,
) -> Option<Run> {
    (!rules::attachment_icon(element)
        && element
            .value()
            .attr("src")
            .is_some_and(|value| crate::safety::safe_url(value, true)))
    .then(|| Run {
        inline_kind: None,
        source_nodes: snapshot_node(snapshot, element.id()),
        line_break: false,
        image_attributes: Some(crate::model::ImageAttributes {
            alt: element.value().attr("alt").map(str::to_string),
            title: element.value().attr("title").map(str::to_string),
            width: rules::style(element, "width")
                .or_else(|| element.value().attr("width").map(str::to_string))
                .or_else(|| crate::word::vml_dimension(element, "width")),
            height: rules::style(element, "height")
                .or_else(|| element.value().attr("height").map(str::to_string))
                .or_else(|| crate::word::vml_dimension(element, "height")),
        }),
        text: String::new(),
        bold: context.bold,
        italic: context.italic,
        underline: context.underline,
        strike: context.strike,
        font_family: context.font_family.clone(),
        font_size: context.font_size.clone(),
        color: context.color.clone(),
        background_color: context.background_color.clone(),
        vertical_align: context.vertical_align.clone(),
        link: context.link.clone(),
        image: element
            .value()
            .attr("src")
            .map(|value| rules::absolute_url(context.base_url.as_ref(), value)),
        source: "original".into(),
    })
}

pub(crate) fn visit_node(
    element: ElementRef,
    context: &RunContext,
    snapshot: &Snapshot,
    output: &mut Vec<Run>,
) {
    if crate::media::bookmark(element) {
        return;
    }
    if crate::media::special(element.value().name()) {
        if matches!(element.value().name(), "svg" | "math") {
            if let Some(mut run) = text_run(&crate::media::alternative(element), context) {
                run.source_nodes = snapshot_node(snapshot, element.id());
                output.push(run);
            }
            return;
        }
        for (url, label) in crate::media::links(element) {
            if let Some(mut run) = text_run(&label, context) {
                run.link = Some(rules::absolute_url(context.base_url.as_ref(), &url));
                run.source_nodes = snapshot_node(snapshot, element.id());
                output.push(run);
            }
        }
        if element.value().name() == "iframe" {
            return;
        }
    }
    if let Some((href, title)) = crate::attachment::embedded_pdf(element) {
        let mut run = text_run(&title, &context.for_element(element)).unwrap_or_else(|| Run {
            inline_kind: None,
            line_break: false,
            text: title,
            bold: context.bold,
            italic: context.italic,
            underline: context.underline,
            strike: context.strike,
            font_family: context.font_family.clone(),
            font_size: context.font_size.clone(),
            color: context.color.clone(),
            background_color: context.background_color.clone(),
            vertical_align: context.vertical_align.clone(),
            link: None,
            image: None,
            image_attributes: None,
            source: "inferred".into(),
            source_nodes: Vec::new(),
        });
        run.link = Some(rules::absolute_url(context.base_url.as_ref(), &href));
        run.source_nodes = snapshot_node(snapshot, element.id());
        output.push(run);
        return;
    }
    if element.value().name() == "br" {
        let mut run = text_run("\n", context).expect("newline is nonempty");
        run.line_break = true;
        run.source_nodes = snapshot_node(snapshot, element.id());
        output.push(run);
        return;
    }
    if matches!(
        element.value().name(),
        "script" | "style" | "iframe" | "object" | "embed"
    ) {
        return;
    }
    let context = context.for_element(element);
    for child in element.children() {
        if let Some(child_element) = ElementRef::wrap(child) {
            if matches!(child_element.value().name(), "img" | "v:imagedata") {
                if let Some(run) = image_run(child_element, &context, snapshot) {
                    output.push(run);
                }
            } else {
                visit_node(child_element, &context, snapshot, output);
            }
        } else if let Node::Text(value) = child.value() {
            if let Some(mut run) = text_run(value, &context) {
                run.source_nodes = snapshot_node(snapshot, child.id());
                output.push(run);
            }
        }
    }
}

/// 把节点映射为原始节点标识；节点不在快照内时返回空列表。
fn snapshot_node(snapshot: &Snapshot, id: ego_tree::NodeId) -> Vec<String> {
    snapshot.id_of(id).map(str::to_owned).into_iter().collect()
}

pub(crate) fn inherited_context(root: ElementRef, base_url: &Option<Url>) -> RunContext {
    let ancestors: Vec<_> = root.ancestors().filter_map(ElementRef::wrap).collect();
    ancestors.into_iter().rev().fold(
        RunContext {
            base_url: base_url.clone(),
            ..Default::default()
        },
        |context, element| context.for_element(element),
    )
}

pub(crate) fn runs(root: ElementRef, base_url: &Option<Url>, snapshot: &Snapshot) -> Vec<Run> {
    let mut output = Vec::new();
    visit_node(
        root,
        &inherited_context(root, base_url),
        snapshot,
        &mut output,
    );
    output
}
