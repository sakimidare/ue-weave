//! Assemble output documents from the selected IR and presentation options.
use crate::{markdown, model::Ir, profile::Settings, renderer, typst};

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Html,
    Markdown,
    Typst,
    Json,
}

#[derive(Clone, clap::Args)]
pub struct Options {
    /// 输出格式，以逗号分隔：html,markdown,typst,json。
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "html,markdown,typst"
    )]
    pub formats: Vec<Format>,
}

#[cfg(test)]
impl Default for Options {
    fn default() -> Self {
        Self {
            formats: vec![Format::Html, Format::Markdown, Format::Typst],
        }
    }
}

impl Options {
    pub fn files(
        &self,
        documents: Documents,
        ir: &Ir,
    ) -> crate::error::Result<Vec<(String, Vec<u8>)>> {
        let mut files = Vec::new();
        if self.formats.contains(&Format::Json) {
            files.push((
                "reconstructed.json".into(),
                serde_json::to_vec_pretty(ir).map_err(|source| {
                    crate::error::RecoverError::Serialize {
                        file: "reconstructed.json".into(),
                        source,
                    }
                })?,
            ));
        }
        for (format, name, content) in [
            (Format::Html, "reconstructed.html", documents.html),
            (Format::Markdown, "reconstructed.md", documents.markdown),
            (Format::Typst, "reconstructed.typ", documents.typst),
        ] {
            if self.formats.contains(&format) {
                files.push((name.into(), content.into_bytes()));
            }
        }
        Ok(files)
    }
}

#[derive(Default)]
pub struct Documents {
    pub html: String,
    pub markdown: String,
    pub typst: String,
}

pub fn render(ir: &Ir, settings: Settings, options: &Options) -> Documents {
    let mut documents = if settings.include_metadata {
        metadata_header(ir)
    } else {
        Documents::default()
    };
    if options.formats.contains(&Format::Html) {
        documents.html.push_str(&renderer::render(ir));
    }
    if options.formats.contains(&Format::Markdown) {
        documents.markdown.push_str(&if settings.native_markdown {
            markdown::render_editable(ir)
        } else {
            markdown::render(ir)
        });
    }
    if options.formats.contains(&Format::Typst) {
        documents.typst.push_str(&typst::render(ir));
    }
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
