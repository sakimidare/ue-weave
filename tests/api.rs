//! 固化公开 API：任何签名、字段名或返回类型的变化都会让本文件编译失败。
//!
//! 修改公开接口时，请同步更新这里，并在提交信息中说明。
#![allow(deprecated)]

use ue_weave::{
    Article, Attachment, Change, Diagnostic, Document, Error, Image, Integrity, Level, Metadata,
    OutputOptions, ParseOptions, Profile, SiteSelectors,
};

#[test]
fn constructors_are_pinned() {
    let _: fn(&str, ParseOptions) -> Result<Article, Error> = Article::from_html;
    let _: fn(&[u8], ParseOptions) -> Result<Article, Error> = Article::from_bytes;
    let _: fn(serde_json::Value) -> Result<Article, Error> = Article::from_json;
    let _: fn(&[u8]) -> Result<Article, Error> = Article::from_json_bytes;
    let _: fn() -> ParseOptions = ParseOptions::new;
    let _: fn() -> SiteSelectors = SiteSelectors::new;
    let _: fn(&[u8]) -> Result<SiteSelectors, Error> = SiteSelectors::from_json;
}

#[test]
fn options_builders_are_pinned() {
    let selectors: SiteSelectors = SiteSelectors::new()
        .content(".Article_Content")
        .title(".Article_Title")
        .date(".Article_PublishDate")
        .source(".Article_Source")
        .author(".Article_Author")
        .publisher(".arti_publisher");
    let options: ParseOptions = ParseOptions::new()
        .selector(".Article_Content")
        .selectors(selectors)
        .source("https://example.org/a")
        .infer(false);
    let _: ParseOptions = options;

    let output: OutputOptions = OutputOptions {
        include_metadata: Some(true),
        ..OutputOptions::new(Profile::Aggressive)
    };
    let _: Option<bool> = output.include_metadata;
    let _: Profile = output.profile;
    let _: Option<bool> = output.repair_corrupt_fonts;
    let _: Option<bool> = output.strip_font_family;
    let _: Option<bool> = output.strip_font_size;
    let _: Option<bool> = output.strip_text_color;
    let _: Option<bool> = output.strip_background_color;
    let _: Option<bool> = output.strip_line_height;
    let _: Option<bool> = output.strip_paragraph_spacing;
    let _: Option<bool> = output.strip_table_dimensions;
    let _: Option<bool> = output.strip_cell_padding;
    let _: Option<bool> = output.strip_image_dimensions;
    let _: Option<bool> = output.strip_fonts;
    let _: Option<bool> = output.strip_colors;
    let _: Option<bool> = output.strip_layout;
    let _: Option<bool> = output.strip_alignment;
    let _: Option<bool> = output.strip_indent;
    let _: Option<bool> = output.trim_leading_spaces;
    let _: Option<bool> = output.drop_empty_paragraphs;
    let _: Option<bool> = output.native_markdown;
    let _: OutputOptions = OutputOptions::default();
}

#[test]
fn accessors_are_pinned() {
    let _: fn(&Article) -> &Metadata = Article::metadata;
    let _: fn(&Article) -> Integrity = Article::integrity;
    let _: fn(&Article) -> &[Diagnostic] = Article::diagnostics;
    let _: fn(&Article) -> &[Attachment] = Article::attachments;
    let _: fn(&Article) -> &[Image] = Article::images;
}

#[test]
fn exporters_are_pinned() {
    let _: fn(&Article) -> String = Article::to_html;
    let _: fn(&Article, OutputOptions) -> String = Article::to_html_with;
    let _: fn(&Article) -> String = Article::to_markdown;
    let _: fn(&Article, OutputOptions) -> String = Article::to_markdown_with;
    let _: fn(&Article) -> String = Article::to_typst;
    let _: fn(&Article, OutputOptions) -> String = Article::to_typst_with;
    let _: fn(&Article) -> Document = Article::to_document;
    let _: fn(&Article, OutputOptions) -> Document = Article::to_document_with;
    let _: fn(&Article) -> serde_json::Value = Article::to_json;
    let _: fn(&Article) -> Vec<u8> = Article::to_json_bytes;
    let _: fn(&Article) -> serde_json::Value = Article::to_json_compact;
    let _: fn(&Article) -> Vec<u8> = Article::to_json_compact_bytes;
}

#[test]
fn deprecated_aliases_are_pinned() {
    let _: fn(&Article) -> String = Article::html;
    let _: fn(&Article) -> String = Article::markdown;
    let _: fn(&Article) -> String = Article::typst;
    let _: fn(&Article) -> serde_json::Value = Article::json;
}

#[test]
fn data_types_are_pinned() {
    fn metadata(value: &Metadata) {
        let _: &Option<String> = &value.title;
        let _: &Option<String> = &value.date;
        let _: &Option<String> = &value.source;
        let _: &Option<String> = &value.author;
        let _: &Option<String> = &value.publisher;
        let _: &Option<String> = &value.url;
    }
    fn attachment(value: &Attachment) {
        let _: &Option<String> = &value.url;
        let _: &Option<String> = &value.text;
        let _: &Option<String> = &value.local_path;
    }
    fn image(value: &Image) {
        let _: &Option<String> = &value.url;
        let _: &Option<String> = &value.alt;
        let _: &Option<String> = &value.local_path;
    }
    fn document(value: &Document) {
        let _: &String = &value.html;
        let _: &String = &value.markdown;
        let _: &String = &value.typst;
        let _: &Vec<Change> = &value.changes;
    }
    fn integrity(value: Integrity) {
        let _: Option<bool> = value.text;
        let _: Option<bool> = value.resources;
    }
    fn diagnostic(value: &Diagnostic) {
        let _: &str = value.code();
        let _: Level = value.level();
        let _: &str = value.kind();
        let _: &[&'static str] = value.topics();
        let _: &str = value.message();
        let _: &serde_json::Value = value.details();
    }
    fn change(value: &Change) {
        let _: &str = value.kind();
        let _: Option<&str> = value.field();
        let _: &serde_json::Value = value.details();
    }
    fn error(value: &Error) {
        let _: &'static str = value.code();
    }
    let _ = (
        metadata, attachment, image, document, integrity, diagnostic, change, error,
    );
}

#[test]
fn profile_variants_are_pinned() {
    for profile in [
        Profile::Conservative,
        Profile::Balanced,
        Profile::Aggressive,
    ] {
        let _: Profile = profile;
    }
    let _: Profile = Profile::default();
}
