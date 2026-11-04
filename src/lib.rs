//! 网页文章的解析、编辑与导出。
#![warn(missing_docs)]

#[cfg(test)]
mod acceptance;
mod asset_format;
mod asset_name;
mod attachment;
mod audit;
mod css;
mod diagnostic;
mod document;
mod downloader;
mod error;
mod explain;
mod export;
mod inference;
mod inline;
mod input;
mod integrity;
mod ir_format;
mod log;
mod markdown;
mod model;
mod network;
mod network_policy;
#[cfg(test)]
mod network_tests;
mod parser;
mod profile;
mod regenerate;
mod renderer;
#[cfg(test)]
mod robustness;
mod rules;
mod safety;
mod site;
mod site_config;
mod snapshot;
mod style_registry;
mod table;
mod typst;
mod word;

pub mod cli;
mod pipeline;

/// 已解析的文章，包含正文、图片、附件和诊断记录。
pub struct Article(model::Ir);

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

impl Article {
    /// 解析指定选择器内的正文；source 用于补全相对链接，infer 控制格式推断。
    ///
    /// 输入 HTML 仅作为文本解析，调用不会发起网络请求。
    ///
    /// ```
    /// use ue_weave::Article;
    /// let article = Article::from_html(
    ///     "<main><p>报名<strong>通知</strong></p></main>",
    ///     "main", None, true,
    /// )?;
    /// assert!(article.html().contains("<strong>通知</strong>"));
    /// # Ok::<(), ue_weave::Error>(())
    /// ```
    pub fn from_html(
        html: &str,
        selector: &str,
        source: Option<&str>,
        infer: bool,
    ) -> Result<Self, Error> {
        let document = scraper::Html::parse_document(html);
        Ok(Self(parser::parse(
            &document,
            selector,
            !infer,
            source.map(str::to_owned),
        )?))
    }

    /// 根据当前正文生成语义化 HTML。
    pub fn html(&self) -> String {
        renderer::render(&self.0)
    }

    /// 根据当前正文生成 Markdown。
    pub fn markdown(&self) -> String {
        markdown::render(&self.0)
    }

    /// 根据当前正文生成 Typst。
    pub fn typst(&self) -> String {
        typst::render(&self.0)
    }

    /// 返回可编辑的 JSON 正文。
    pub fn json(&self) -> serde_json::Value {
        document::from_analysis(&self.0)
    }
}
