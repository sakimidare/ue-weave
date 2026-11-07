//! 网页文章的解析、编辑与导出。
//!
//! 最小用法：
//!
//! ```
//! use ue_weave::{Article, Options};
//!
//! let article = Article::from_html(
//!     "<main><p>报名<strong>通知</strong></p></main>",
//!     Options::new().selector("main"),
//! )?;
//! assert!(article.to_html().contains("<strong>通知</strong>"));
//! assert_eq!(article.metadata().title, None);
//! # Ok::<(), ue_weave::Error>(())
//! ```
#![warn(missing_docs)]

#[cfg(test)]
mod acceptance;
mod asset_format;
mod asset_name;
mod attachment;
mod audit;
mod code;
mod css;
mod depth;
mod diagnostic;
mod document;
mod downloader;
mod error;
mod explain;
mod export;
mod html;
mod inference;
mod inline;
mod input;
mod integrity;
mod ir_format;
mod log;
mod markdown;
mod media;
mod model;
mod network;
mod network_policy;
#[cfg(test)]
mod network_tests;
mod output_runs;
mod parser;
mod profile;
mod regenerate;
mod renderer;
mod report;
#[cfg(test)]
mod robustness;
mod rules;
mod safety;
#[cfg(test)]
mod semantics_tests;
mod site;
mod site_config;
mod snapshot;
mod style_registry;
mod table;
mod tag_registry;
mod typst;
mod word;

#[doc(hidden)]
pub mod cli;
mod pipeline;

pub use profile::Profile;

use std::path::Path;

/// 解析选项。
///
/// 默认自动识别正文容器、启用格式推断、不补全链接。
///
/// 字段私有并使用链式方法：这里只有三个取值稳定的选项，构建器能避免
/// 外部代码依赖字段布局，后续新增选项也不会破坏调用方。
#[derive(Clone, Debug)]
pub struct Options {
    selector: Option<String>,
    source: Option<String>,
    infer: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

impl Options {
    /// 使用默认选项。
    pub fn new() -> Self {
        Self {
            selector: None,
            source: None,
            infer: true,
        }
    }

    /// 指定正文容器的 CSS 选择器；省略时自动识别。
    pub fn selector(mut self, selector: impl Into<String>) -> Self {
        self.selector = Some(selector.into());
        self
    }

    /// 原网页网址，用于把相对链接补成绝对地址，并作为元数据来源。
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// 是否根据文本和样式推断标题、署名和缩进。
    pub fn infer(mut self, infer: bool) -> Self {
        self.infer = infer;
        self
    }
}

/// 文档输出选项。
///
/// 默认使用保守预设，所有细分字段为 `None`，表示跟随预设。
/// 字段公开并使用结构体更新语法：开关数量多，`None` 表示跟随预设，
/// 直接列出字段比十多个链式方法更容易发现和对照文档。
///
/// ```
/// use ue_weave::{OutputOptions, Profile};
/// let options = OutputOptions {
///     profile: Profile::Aggressive,
///     strip_fonts: Some(false),
///     include_metadata: Some(true),
///     ..OutputOptions::default()
/// };
/// ```
#[derive(Clone, Debug, Default)]
pub struct OutputOptions {
    /// 输出预设。
    pub profile: Profile,
    /// 在正文顶部加入标题、日期、作者、来源和发布者。
    pub include_metadata: Option<bool>,
    /// 修复编辑器产生的损坏字体名；`None` 跟随预设（默认修复）。
    pub repair_corrupt_fonts: Option<bool>,
    /// 移除字体名称。
    pub strip_font_family: Option<bool>,
    /// 移除字号。
    pub strip_font_size: Option<bool>,
    /// 移除文字颜色。
    pub strip_text_color: Option<bool>,
    /// 移除背景色。
    pub strip_background_color: Option<bool>,
    /// 移除行距。
    pub strip_line_height: Option<bool>,
    /// 移除段间距。
    pub strip_paragraph_spacing: Option<bool>,
    /// 移除表格、列和单元格固定尺寸。
    pub strip_table_dimensions: Option<bool>,
    /// 移除单元格内边距。
    pub strip_cell_padding: Option<bool>,
    /// 移除图片显式宽高。
    pub strip_image_dimensions: Option<bool>,
    /// 移除字体和字号，覆盖 `strip_font_family` 和 `strip_font_size`。
    pub strip_fonts: Option<bool>,
    /// 移除文字颜色和背景色。
    pub strip_colors: Option<bool>,
    /// 移除行距、段间距、表格尺寸和单元格内边距。
    pub strip_layout: Option<bool>,
    /// 移除显式文本对齐。
    pub strip_alignment: Option<bool>,
    /// 移除首行缩进声明。
    pub strip_indent: Option<bool>,
    /// 清理段落、标题和署名开头的排版空格。
    pub trim_leading_spaces: Option<bool>,
    /// 移除纯空白段落。
    pub drop_empty_paragraphs: Option<bool>,
    /// 简单列表和表格优先使用原生 Markdown。
    pub native_markdown: Option<bool>,
}

impl OutputOptions {
    /// 使用指定预设，其余细分字段跟随预设。
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            ..Self::default()
        }
    }

    fn resolve(&self) -> profile::Settings {
        profile::Overrides {
            include_metadata: self.include_metadata,
            no_repair_corrupt_fonts: self.repair_corrupt_fonts == Some(false),
            strip_font_family: self.strip_font_family,
            strip_font_size: self.strip_font_size,
            strip_text_color: self.strip_text_color,
            strip_background_color: self.strip_background_color,
            strip_line_height: self.strip_line_height,
            strip_paragraph_spacing: self.strip_paragraph_spacing,
            strip_table_dimensions: self.strip_table_dimensions,
            strip_cell_padding: self.strip_cell_padding,
            strip_image_dimensions: self.strip_image_dimensions,
            strip_fonts: self.strip_fonts,
            strip_colors: self.strip_colors,
            strip_layout: self.strip_layout,
            strip_alignment: self.strip_alignment,
            strip_indent: self.strip_indent,
            trim_leading_spaces: self.trim_leading_spaces,
            drop_empty_paragraphs: self.drop_empty_paragraphs,
            native_markdown: self.native_markdown,
        }
        .resolve(self.profile)
    }
}

/// 同时生成的三种文档。
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Document {
    /// 语义化 HTML。
    pub html: String,
    /// Markdown。
    pub markdown: String,
    /// Typst。
    pub typst: String,
    /// 导出阶段的格式变更，例如被剥离的字体或补全的空表头。
    pub changes: Vec<Change>,
}

/// 一条导出变更，与命令行 `output_changes` 对应。
#[derive(Clone)]
pub struct Change {
    details: serde_json::Value,
}

impl Change {
    /// 变更类型，例如 `corrupt-font-repair`。
    pub fn kind(&self) -> &str {
        self.details
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    }

    /// 变更涉及的字段，例如 `runs.font_family`。
    pub fn field(&self) -> Option<&str> {
        self.details
            .get("field")
            .and_then(serde_json::Value::as_str)
    }

    /// 完整详情。
    pub fn details(&self) -> &serde_json::Value {
        &self.details
    }
}

impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Change")
            .field("kind", &self.kind())
            .field("field", &self.field())
            .finish()
    }
}

/// 文章标题、日期、来源、作者和发布者。
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Metadata {
    /// 标题。
    pub title: Option<String>,
    /// 发布日期。
    pub date: Option<String>,
    /// 文章来源。
    pub source: Option<String>,
    /// 作者。
    pub author: Option<String>,
    /// 发布者。
    pub publisher: Option<String>,
    /// 原网页地址。
    pub url: Option<String>,
}

/// 诊断级别。
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// 需要检查的警告。
    Warning,
    /// 处理提示。
    Note,
}

impl Level {
    /// 命令行日志中的名称。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 一条诊断，编号与命令行日志一致。
#[derive(Clone)]
pub struct Diagnostic {
    details: serde_json::Value,
    message: String,
    level: Level,
    topics: Vec<&'static str>,
}

impl Diagnostic {
    fn from_internal(diagnostic: &crate::diagnostic::Diagnostic) -> Option<Self> {
        Some(Self {
            details: serde_json::to_value(diagnostic).ok()?,
            message: diagnostic.kind.to_string(),
            level: match diagnostic.kind.level() {
                "warning" => Level::Warning,
                _ => Level::Note,
            },
            topics: crate::diagnostic::topics(diagnostic.kind)
                .iter()
                .map(|topic| topic.name())
                .collect(),
        })
    }

    /// 稳定编号，例如 `W0014`，可用 `--explain` 查询。
    pub fn code(&self) -> &str {
        self.details
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    }

    /// 级别。
    pub fn level(&self) -> Level {
        self.level
    }

    /// 诊断类型名，例如 `unmapped-style`。
    pub fn kind(&self) -> &str {
        self.details
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    }

    /// 所属日志领域，与 `--log` 的 topic 一致。涉及多领域时返回多个名称。
    pub fn topics(&self) -> &[&'static str] {
        &self.topics
    }

    /// 中文描述。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 完整详情。
    pub fn details(&self) -> &serde_json::Value {
        &self.details
    }
}

impl std::fmt::Debug for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Diagnostic")
            .field("code", &self.code())
            .field("level", &self.level())
            .field("message", &self.message)
            .finish()
    }
}

fn parse_document(document: &scraper::Html, options: &Options) -> Result<model::Ir, Error> {
    let requested = options.selector.as_deref().unwrap_or("auto");
    let selector = site::content_selector(document, requested)?;
    Ok(parser::parse(
        document,
        &selector,
        !options.infer,
        options.source.clone(),
    )?)
}

/// 文章中的附件。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Attachment {
    /// 附件原始地址；下载后仍保留远程 URL。
    pub url: Option<String>,
    /// 附件显示文字。
    pub text: Option<String>,
    /// 本地文件路径，未下载时为 None。
    pub local_path: Option<String>,
}

/// 文章中的图片。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Image {
    /// 图片原始地址；下载后仍保留远程 URL。
    pub url: Option<String>,
    /// 替代文字。
    pub alt: Option<String>,
    /// 本地文件路径，未下载时为 None。
    pub local_path: Option<String>,
}

fn text_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn meta_field(metadata: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// 已解析的文章，包含正文、元数据、图片、附件和诊断记录。
#[derive(Clone)]
pub struct Article {
    ir: model::Ir,
    metadata: Metadata,
    diagnostics: Vec<Diagnostic>,
    attachments: Vec<Attachment>,
    images: Vec<Image>,
}

impl std::fmt::Debug for Article {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Article")
            .field("metadata", &self.metadata)
            .field("blocks", &self.ir.blocks.len())
            .field("diagnostics", &self.ir.warnings.len())
            .finish()
    }
}

impl Article {
    fn new(mut ir: model::Ir, warnings: Vec<crate::diagnostic::Diagnostic>) -> Self {
        ir.warnings.extend(warnings);
        let metadata = Metadata {
            title: meta_field(&ir.metadata, "title"),
            date: meta_field(&ir.metadata, "date"),
            source: meta_field(&ir.metadata, "source"),
            author: meta_field(&ir.metadata, "author"),
            publisher: meta_field(&ir.metadata, "publisher"),
            url: ir
                .source
                .clone()
                .or_else(|| meta_field(&ir.metadata, "url")),
        };
        let diagnostics = ir
            .warnings
            .iter()
            .filter_map(Diagnostic::from_internal)
            .collect();
        let attachments = ir
            .attachments
            .iter()
            .map(|value| Attachment {
                url: text_field(value, "remote_href").or_else(|| text_field(value, "href")),
                text: text_field(value, "text"),
                local_path: text_field(value, "local_path"),
            })
            .collect();
        let images = ir
            .images
            .iter()
            .map(|value| Image {
                url: text_field(value, "remote_src").or_else(|| text_field(value, "src")),
                alt: text_field(value, "alt"),
                local_path: text_field(value, "local_path"),
            })
            .collect();
        Self {
            ir,
            metadata,
            diagnostics,
            attachments,
            images,
        }
    }

    /// 解析 HTML 文本。
    ///
    /// ```
    /// use ue_weave::{Article, Options};
    /// let article = Article::from_html("<div class='Article_Content'><p>正文</p></div>", Options::new())?;
    /// assert!(article.to_markdown().contains("正文"));
    /// # Ok::<(), ue_weave::Error>(())
    /// ```
    pub fn from_html(html: &str, options: Options) -> Result<Self, Error> {
        let document = crate::html::parse(html);
        Ok(Self::new(parse_document(&document, &options)?, Vec::new()))
    }

    /// 解析页面字节，按 BOM、HTTP 声明或 meta 标签识别编码。
    pub fn from_bytes(bytes: &[u8], options: Options) -> Result<Self, Error> {
        let input = input::Input::decode(bytes.to_vec(), "<bytes>".into(), None);
        let document = crate::html::parse(&input.html);
        Ok(Self::new(
            parse_document(&document, &options)?,
            input.warnings,
        ))
    }

    /// 读取 `to_json()` 或 `to_json_compact()` 的结果。
    pub fn from_json(value: serde_json::Value) -> Result<Self, Error> {
        let ir = ir_format::from_value(value, Path::new("<json>"))?;
        Ok(Self::new(ir, Vec::new()))
    }

    /// 读取 JSON 字节，例如 `to_json_bytes()` 或 `to_json_compact_bytes()` 的结果。
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let ir = ir_format::read(bytes, Path::new("<json>"))?;
        Ok(Self::new(ir, Vec::new()))
    }

    /// 文章元数据。
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// 解析和导出过程中的诊断。
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// 文章附件。
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// 文章图片。
    pub fn images(&self) -> &[Image] {
        &self.images
    }

    /// 按默认选项生成语义化 HTML；等同命令行的默认输出。
    pub fn to_html(&self) -> String {
        self.to_html_with(OutputOptions::default())
    }

    /// 按指定选项生成语义化 HTML。
    pub fn to_html_with(&self, options: OutputOptions) -> String {
        self.render(options.resolve(), vec![export::Format::Html], false)
            .0
            .html
    }

    /// 按默认选项生成 Markdown，保留字体、颜色和 HTML 表格。
    pub fn to_markdown(&self) -> String {
        self.to_markdown_with(OutputOptions::default())
    }

    /// 按指定选项生成 Markdown；激进档使用原生表格和列表。
    pub fn to_markdown_with(&self, options: OutputOptions) -> String {
        self.render(options.resolve(), vec![export::Format::Markdown], false)
            .0
            .markdown
    }

    /// 按默认选项生成 Typst。
    pub fn to_typst(&self) -> String {
        self.to_typst_with(OutputOptions::default())
    }

    /// 按指定选项生成 Typst。
    pub fn to_typst_with(&self, options: OutputOptions) -> String {
        self.render(options.resolve(), vec![export::Format::Typst], false)
            .0
            .typst
    }

    /// 一次生成三种文档，行为与命令行默认输出一致。
    ///
    /// ```
    /// use ue_weave::{Article, Options, OutputOptions, Profile};
    /// let article = Article::from_html("<div class='Article_Content'><p>正文</p></div>", Options::new())?;
    /// let document = article.to_document(OutputOptions {
    ///     include_metadata: Some(true),
    ///     ..OutputOptions::new(Profile::Aggressive)
    /// });
    /// assert!(document.markdown.contains("正文"));
    /// # Ok::<(), ue_weave::Error>(())
    /// ```
    pub fn to_document(&self, options: OutputOptions) -> Document {
        let settings = options.resolve();
        let (documents, changes) = self.render(
            settings,
            vec![
                export::Format::Html,
                export::Format::Markdown,
                export::Format::Typst,
            ],
            true,
        );
        Document {
            html: documents.html,
            markdown: documents.markdown,
            typst: documents.typst,
            changes: changes
                .into_iter()
                .map(|details| Change { details })
                .collect(),
        }
    }

    /// 应用预设后按需生成格式，保证单格式与整包输出走同一条路径。
    fn render(
        &self,
        settings: profile::Settings,
        formats: Vec<export::Format>,
        record_changes: bool,
    ) -> (export::Documents, Vec<serde_json::Value>) {
        let prepared = if record_changes {
            profile::prepare(&self.ir, settings)
        } else {
            profile::prepare_documents(&self.ir, settings)
        };
        let documents = export::render(&prepared.ir, settings, &export::Options { formats });
        (documents, prepared.changes)
    }

    /// 生成可编辑的 JSON 正文，可交给 `from_json` 重新读取。
    pub fn to_json(&self) -> serde_json::Value {
        document::from_analysis(&self.ir)
    }

    /// 生成可编辑的 JSON 字节，可直接交给 `from_json_bytes`，无需额外引入 serde_json。
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.to_json()).expect("正文 JSON 可序列化")
    }

    /// 生成省略来源节点的 JSON，体积明显更小，仍可交给 `from_json`。
    ///
    /// 代价是无法再追溯到原始 HTML 节点，`diagnostics()` 中的 `source_nodes` 也会为空。
    pub fn to_json_compact(&self) -> serde_json::Value {
        document::from_analysis_compact(&self.ir)
    }

    /// 生成省略来源节点的 JSON 字节。
    pub fn to_json_compact_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.to_json_compact()).expect("正文 JSON 可序列化")
    }

    #[deprecated(since = "0.2.0", note = "使用 to_html")]
    /// 兼容旧名，等同 `to_html`。
    pub fn html(&self) -> String {
        self.to_html()
    }

    #[deprecated(since = "0.2.0", note = "使用 to_markdown")]
    /// 兼容旧名，等同 `to_markdown`。
    pub fn markdown(&self) -> String {
        self.to_markdown()
    }

    #[deprecated(since = "0.2.0", note = "使用 to_typst")]
    /// 兼容旧名，等同 `to_typst`。
    pub fn typst(&self) -> String {
        self.to_typst()
    }

    #[deprecated(since = "0.2.0", note = "使用 to_json")]
    /// 兼容旧名，等同 `to_json`。
    pub fn json(&self) -> serde_json::Value {
        self.to_json()
    }
}

/// 文章处理失败；编号与命令行日志一致。
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(#[from] error::RecoverError);

impl Error {
    /// 返回稳定的错误编号，可用命令行 --explain 查询。
    pub fn code(&self) -> &'static str {
        self.0.code()
    }
}
