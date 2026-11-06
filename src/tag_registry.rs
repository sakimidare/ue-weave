//! 解析和诊断共用的标签处理方式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Handling {
    Block,
    Inline,
    Container,
    Partial,
    Excluded,
    Unknown,
}

pub fn handling(tag: &str) -> Handling {
    match tag {
        "p" | "li" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "table"
        | "blockquote" | "pre" | "hr" | "dl" | "dt" | "dd" => Handling::Block,
        "sdfield" | "span" | "strong" | "b" | "em" | "i" | "u" | "s" | "strike" | "del" | "sup"
        | "sub" | "font" | "br" | "a" | "img" | "code" | "kbd" | "samp" | "var" | "thead"
        | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" | "col" | "colgroup" => {
            Handling::Inline
        }
        "div" | "section" | "article" | "header" | "footer" | "main" | "aside" | "nav"
        | "center" | "noscript" => Handling::Container,
        "figure" | "figcaption" | "address" | "fieldset" | "legend" | "details" | "summary"
        | "svg" | "math" | "iframe" | "video" | "audio" | "source" => Handling::Partial,
        "script" | "style" | "object" | "embed" => Handling::Excluded,
        _ => Handling::Unknown,
    }
}

pub fn block(tag: &str) -> bool {
    handling(tag) == Handling::Block
}
pub fn container(tag: &str) -> bool {
    handling(tag) == Handling::Container
        || matches!(
            tag,
            "figure" | "figcaption" | "address" | "fieldset" | "legend" | "details" | "summary"
        )
}
