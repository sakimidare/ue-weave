//! Assemble output documents from the selected IR and presentation options.
use crate::{markdown, model::Ir, profile::Settings, renderer, typst};

#[derive(Default)]
pub struct Documents {
    pub html: String,
    pub markdown: String,
    pub typst: String,
}

pub fn render(ir: &Ir, settings: Settings) -> Documents {
    let mut documents = if settings.include_metadata {
        metadata_header(ir)
    } else {
        Documents::default()
    };
    documents.html.push_str(&renderer::render(ir));
    documents.markdown.push_str(&if settings.native_markdown {
        markdown::render_editable(ir)
    } else {
        markdown::render(ir)
    });
    documents.typst.push_str(&typst::render(ir));
    documents
}

fn metadata_header(ir: &Ir) -> Documents {
    let mut header = Documents::default();
    for (key, label) in [
        ("title", ""),
        ("date", "日期"),
        ("author", "作者"),
        ("source", "来源"),
        ("publisher", "发布者"),
    ] {
        let Some(value) = ir.metadata.get(key).and_then(|v| v.as_str()) else {
            continue;
        };
        let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if value.is_empty() {
            continue;
        }
        if key == "title" {
            header
                .html
                .push_str(&format!("<h1>{}</h1>\n", html_escape::encode_text(&value)));
            header
                .markdown
                .push_str(&format!("# {}\n\n", markdown::escape_text(&value)));
            header.typst.push_str(&format!(
                "#heading(level: 1, outlined: false)[#text({})]\n\n",
                typst::quoted(&value)
            ));
        } else {
            let text = format!("{label}：{value}");
            header
                .html
                .push_str(&format!("<p>{}</p>\n", html_escape::encode_text(&text)));
            header
                .markdown
                .push_str(&format!("{}\n\n", markdown::escape_text(&text)));
            header
                .typst
                .push_str(&format!("#text({})\n\n", typst::quoted(&text)));
        }
    }
    if !header.html.is_empty() {
        header.html = format!(
            "<header class=\"article-metadata\">\n{}</header>\n",
            header.html
        );
    }
    header
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_metadata_is_omitted_and_values_are_escaped() {
        let mut ir = Ir::default();
        ir.metadata.insert("date".into(), serde_json::json!("  "));
        ir.metadata
            .insert("author".into(), serde_json::json!({"value":"ignored"}));
        assert!(metadata_header(&ir).html.is_empty());
        ir.metadata.insert(
            "title".into(),
            serde_json::json!("<script>*x*</script>\n#hello"),
        );
        ir.metadata
            .insert("source".into(), serde_json::json!("\"quote\" \\ code"));
        let header = metadata_header(&ir);
        assert!(!header.html.contains("<script>"));
        assert!(header.html.contains("&lt;script&gt;"));
        assert!(header.markdown.contains(r"\*x\*"));
        assert!(header.markdown.contains(r"\#hello"));
        assert!(header.typst.contains(r#"\"quote\" \\ code"#));
        assert!(!header.html.contains("日期"));
        assert!(!header.html.contains("作者"));
    }
}
