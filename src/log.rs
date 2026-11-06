use crate::diagnostic::{Details, Diagnostic, NoteKind};
use anstyle::{AnsiColor, Style};
use std::io::Write;

mod context;
mod filter;
mod layout;
mod output_context;
mod render;

pub use filter::{Filter, Level};

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum Color {
    #[default]
    Auto,
    Always,
    Never,
}

impl std::str::FromStr for Color {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(()),
        }
    }
}

impl Color {
    pub(crate) fn choice(self) -> anstream::ColorChoice {
        match self {
            Self::Always => anstream::ColorChoice::Always,
            Self::Never => anstream::ColorChoice::Never,
            Self::Auto => anstream::ColorChoice::Auto,
        }
    }
}

pub struct Logger {
    filter: Filter,
    stream: std::cell::RefCell<anstream::AutoStream<std::io::Stderr>>,
    width: usize,
}

impl Logger {
    pub fn retry(&self, error: &crate::error::NetworkError, delay: std::time::Duration) {
        let diagnostic = Diagnostic::new(
            NoteKind::NetworkRetry,
            Details {
                url: Some((error.url).to_owned()),
                error_code: Some((error.kind.code()).to_owned()),
                error: Some((error.to_string()).to_owned()),
                retry: Some((format!("{}ms 后重试", delay.as_millis())).to_owned()),
                ..Default::default()
            },
        );
        self.emit_enabled(&diagnostic);
    }
    /// 只在诊断未被过滤时输出，供运行时提示和重试信息使用。
    fn emit_enabled(&self, diagnostic: &Diagnostic) {
        if self.filter.level(diagnostic.kind) == Level::Off {
            return;
        }
        self.emit(&detail(diagnostic, &[]));
    }
    #[cfg(test)]
    pub fn new(verbose: bool) -> Self {
        Self::with_filter(
            Filter::from_specs(&[], false, verbose).unwrap(),
            Color::Auto,
        )
    }
    pub fn with_filter(filter: Filter, color: Color) -> Self {
        Self {
            filter,
            stream: std::cell::RefCell::new(anstream::AutoStream::new(
                std::io::stderr(),
                color.choice(),
            )),
            width: layout::width(),
        }
    }
    fn emit(&self, text: &str) {
        self.write(&format_message(text));
    }
    fn write(&self, text: &str) {
        let _ = self.stream.borrow_mut().write_all(text.as_bytes());
    }
    pub fn error(&self, error: &crate::error::RecoverError) {
        self.emit(&format!("error[{}]: {error}\n", error.code()));
        self.explain_hint(error.code());
    }
    fn explain_hint(&self, code: &str) {
        if self.filter.level(NoteKind::ExplainHint.into()) == Level::Off {
            return;
        }
        self.emit(&format!(
            "hint[{}]: 运行 `ue-weave --explain {code}` 查看解释和处理方法。\n",
            NoteKind::ExplainHint.code()
        ));
    }
    pub fn finish_diagnostics(&self, diagnostics: &[Diagnostic]) {
        if let Some(warning) = diagnostics.iter().find(|diagnostic| {
            self.filter.level(diagnostic.kind) != Level::Off
                && matches!(
                    diagnostic.kind,
                    crate::diagnostic::DiagnosticKind::Warning(_)
                )
        }) {
            self.explain_hint(warning.kind.code());
        }
    }
    pub fn stage(&self, name: &str, message: impl AsRef<str>) {
        if self.filter.progress() != Level::Off {
            let label = match name {
                "input" | "input-ir" => "Reading",
                "request" => "Fetching",
                "parse" => "Parsing",
                "output" => "Writing",
                "done" => "Finished",
                _ => "Processing",
            };
            let message: String = message
                .as_ref()
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            let label = format!("{label:>12}");
            let style = AnsiColor::Green.on_default().bold();
            self.write(&format!("{style}{label}{style:#} {message}\n"));
        }
    }
    pub fn done(&self, blocks: usize, attachments: usize, images: usize) {
        self.stage(
            "done",
            format!("完成：{blocks} 个正文块，{attachments} 个附件，{images} 张图片"),
        );
    }
    pub fn warnings(
        &self,
        warnings: &[crate::diagnostic::Diagnostic],
        nodes: &[serde_json::Value],
        output: &crate::model::Ir,
    ) {
        if warnings.is_empty() {
            return;
        }
        let levels: Vec<Level> = warnings
            .iter()
            .map(|warning| self.filter.level(warning.kind))
            .collect();
        if levels.iter().all(|level| *level == Level::Off) {
            return;
        }
        let context = levels
            .contains(&Level::Context)
            .then(|| context::Context::new(nodes));
        let output_context = context
            .as_ref()
            .map(|context| output_context::OutputContext::new(output, context));
        let mut counts = std::collections::BTreeMap::new();
        for (warning, level) in warnings.iter().zip(&levels) {
            match level {
                Level::Off => {}
                Level::Summary => {
                    *counts.entry(warning.kind).or_insert(0usize) += 1;
                }
                Level::List => self.emit(&detail(warning, nodes)),
                Level::Context => match (&context, &output_context) {
                    (Some(context), Some(output)) => {
                        self.write(&render::diagnostic(warning, context, output, self.width))
                    }
                    _ => self.emit(&detail(warning, nodes)),
                },
            }
        }
        if !counts.is_empty() {
            for (kind, count) in counts {
                self.emit(&format!(
                    "{}[{}]: {kind}（{count} 处）\n",
                    kind.level(),
                    kind.code()
                ));
            }
            self.emit_enabled(&crate::diagnostic::Diagnostic::new(
                crate::diagnostic::NoteKind::VerboseHint,
                crate::diagnostic::Details::default(),
            ));
        }
    }
}

fn format_message(text: &str) -> String {
    let mut output = String::new();
    for (index, line) in text.lines().enumerate() {
        let line: String = line.chars().filter(|c| !c.is_control()).collect();
        let (prefix, rest, shade) = if index == 0 {
            let split = line.find(':').map(|i| i + 1).unwrap_or(line.len());
            let shade = if line.starts_with("error") {
                AnsiColor::Red
            } else if line.starts_with("warning") {
                AnsiColor::Yellow
            } else {
                AnsiColor::Cyan
            };
            (line[..split].to_string(), line[split..].to_string(), shade)
        } else if let Some(rest) = line.strip_prefix("  --> ") {
            ("  --> ".into(), rest.into(), AnsiColor::Blue)
        } else {
            (
                "   | ".into(),
                line.trim_start_matches("  = ").into(),
                AnsiColor::Blue,
            )
        };
        let style: Style = shade.on_default().bold();
        output.push_str(&format!("{style}{prefix}{style:#}{rest}\n"));
    }
    output.push('\n');
    output
}

fn compact(value: &serde_json::Value) -> String {
    let text = value.to_string();
    let mut chars = text.chars();
    let result: String = chars.by_ref().take(180).collect();
    if chars.next().is_some() {
        format!("{result}…")
    } else {
        result
    }
}

fn detail(warning: &crate::diagnostic::Diagnostic, nodes: &[serde_json::Value]) -> String {
    use std::fmt::Write;
    let kind = warning.kind;
    let mut output = format!("{}[{}]: {kind}\n", kind.level(), kind.code());
    if let Some(url) = warning["url"].as_str() {
        let _ = writeln!(output, "  --> {url}");
    }
    if let Some(ids) = warning["source_nodes"].as_array() {
        for id in ids {
            if let Some(node) = nodes.iter().find(|node| node["id"] == *id) {
                let tag = node["tag"].as_str().unwrap_or("text");
                let _ = writeln!(output, "  --> {} <{tag}>", id.as_str().unwrap_or(""));
                if let Some(text) = node
                    .get("text")
                    .filter(|text| text.as_str().is_some_and(|s| !s.trim().is_empty()))
                {
                    let _ = writeln!(output, "  = 文字: {}", compact(text));
                }
            }
        }
    }
    for (label, value) in fields(warning) {
        let _ = writeln!(output, "  = {label}: {value}");
    }
    output
}

fn fields(warning: &crate::diagnostic::Diagnostic) -> Vec<(&'static str, String)> {
    [
        ("text", "段落"),
        ("align", "原对齐"),
        ("attribute", "属性"),
        ("original", "原值"),
        ("value", "结果"),
        ("expected", "预期"),
        ("actual", "实际"),
        ("original_excerpt", "原文片段"),
        ("reconstructed_excerpt", "输出片段"),
        ("error_code", "错误编号"),
        ("error", "错误"),
        ("retry", "retry"),
    ]
    .into_iter()
    .filter_map(|(key, label)| Some((label, compact(warning.get(key).filter(|v| !v.is_null())?))))
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn styled_details_align_and_escape_external_controls() {
        let input = "warning[W0001]: 编码\n  --> page.html\n  = 原值: abc\u{1b}[31m\n";
        use std::io::Write;
        let styled = super::format_message(input);
        let mut stream = anstream::AutoStream::new(Vec::new(), anstream::ColorChoice::Never);
        stream.write_all(styled.as_bytes()).unwrap();
        let plain = String::from_utf8(stream.into_inner()).unwrap();
        assert!(plain.contains("  --> page.html\n   | 原值:"));
        assert!(!plain.contains('\u{1b}'));
        let style = anstyle::AnsiColor::Yellow.on_default().bold();
        assert!(styled.starts_with(&format!("{style}warning[W0001]:{style:#}")));
        assert!(!styled.contains("abc\u{1b}"));
    }
    #[test]
    fn details_show_article_values_instead_of_rule_text() {
        let record = crate::diagnostic::Diagnostic::new(
            crate::diagnostic::NoteKind::SignatureInference,
            crate::diagnostic::Details {
                text: Some("教务处".to_owned()),
                align: Some("right".to_owned()),
                value: serde_json::json!("signature"),
                reason: Some("generic rule".to_owned()),
                ..Default::default()
            },
        );
        let text = super::detail(&record, &[]);
        assert!(text.contains("note[N0002]"));
        assert!(text.contains("教务处"));
        assert!(text.contains("right"));
        assert!(!text.contains("generic rule"));
        assert!(!text.contains("--verbose"));
    }
}
