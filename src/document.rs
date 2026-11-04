//! 可编辑文章格式；解析诊断保存在内部分析数据中。
use crate::model::Ir;
use serde_json::{json, Map, Value};

const METADATA: &[&str] = &["title", "date", "author", "source", "publisher", "url"];
const ASSET: &[&str] = &[
    "href",
    "src",
    "text",
    "title",
    "alt",
    "local_path",
    "source_nodes",
];
const DIAGNOSTICS: &[&str] = &[
    "reason",
    "confidence",
    "raw_html",
    "source",
    "word_structure",
    "signature",
    "warnings",
    "original_nodes",
    "download",
];

fn selected(value: &Value, keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .filter_map(|key| {
                value
                    .get(*key)
                    .filter(|value| !value.is_null())
                    .map(|value| ((*key).into(), value.clone()))
            })
            .collect(),
    )
}

fn content(value: Value) -> Value {
    match value {
        Value::Object(mut object) => {
            if object.contains_key("confidence") && object.contains_key("value") {
                return content(object.remove("value").unwrap());
            }
            object.retain(|key, _| !DIAGNOSTICS.contains(&key.as_str()));
            if object.contains_key("runs")
                && object.get("type").and_then(Value::as_str) != Some("code_block")
            {
                object.remove("text");
            }
            Value::Object(
                object
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let value = content(value);
                        let empty = value.is_null()
                            || value == false
                            || value.as_array().is_some_and(Vec::is_empty)
                            || (value.as_object().is_some_and(Map::is_empty) && key != "list")
                            || value.as_str().is_some_and(|text| {
                                text.is_empty()
                                    || (matches!(key.as_str(), "align" | "indent")
                                        && matches!(text, "unknown" | "none"))
                            });
                        (!empty).then_some((key, value))
                    })
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(content).collect()),
        other => other,
    }
}

pub fn from_analysis(ir: &Ir) -> Value {
    let mut metadata = selected(&json!(ir.metadata), METADATA);
    if let Some(source) = &ir.source {
        metadata["url"] = json!(source);
    }
    let mut document = json!({
        "schema": {"name": "ue-weave-ir", "version": 2},
        "metadata": metadata,
        "blocks": content(json!(ir.blocks)),
    });
    if !ir.original_nodes.is_empty() {
        document["original_nodes"] = json!(ir.original_nodes);
    }
    for (key, assets) in [("images", &ir.images), ("attachments", &ir.attachments)] {
        if !assets.is_empty() {
            document[key] =
                Value::Array(assets.iter().map(|asset| selected(asset, ASSET)).collect());
        }
    }
    document
}

pub fn validate(value: &Value) -> crate::error::Result<()> {
    fn reject(path: String) -> crate::error::RecoverError {
        crate::error::RecoverError::InvalidIr {
            field: path,
            reason: "此字段不属于文章内容".into(),
        }
    }
    fn blocks(value: &Value, path: &str) -> crate::error::Result<()> {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    let path = format!("{path}.{key}");
                    if DIAGNOSTICS.contains(&key.as_str()) {
                        return Err(reject(path));
                    }
                    blocks(value, &path)?;
                }
            }
            Value::Array(array) => {
                for (i, value) in array.iter().enumerate() {
                    blocks(value, &format!("{path}[{i}]"))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    if let Some(object) = value.as_object() {
        for key in object.keys() {
            if ![
                "schema",
                "metadata",
                "blocks",
                "images",
                "attachments",
                "original_nodes",
            ]
            .contains(&key.as_str())
            {
                return Err(reject(key.clone()));
            }
        }
    }
    if let Some(metadata) = value["metadata"].as_object() {
        for key in metadata.keys() {
            if !METADATA.contains(&key.as_str()) {
                return Err(reject(format!("metadata.{key}")));
            }
        }
    }
    blocks(&value["blocks"], "blocks")?;
    for key in ["images", "attachments"] {
        if let Some(assets) = value[key].as_array() {
            for (i, asset) in assets.iter().enumerate() {
                if let Some(asset) = asset.as_object() {
                    for field in asset.keys() {
                        if !ASSET.contains(&field.as_str()) {
                            return Err(reject(format!("{key}[{i}].{field}")));
                        }
                    }
                }
            }
        }
    }
    crate::snapshot::validate(value)
}

// 将文章字段转换为渲染器使用的格式值表示。
pub fn expand(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(expand),
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                if matches!(
                    key.as_str(),
                    "indent" | "level" | "line_height" | "width" | "custom_style"
                ) && !value.is_null()
                    && !value.is_object()
                {
                    // 图片宽度使用字符串，列宽使用 Evidence。
                    if key != "width" {
                        *value = json!({"value": value.take()});
                        continue;
                    }
                }
                if matches!(key.as_str(), "spacing" | "table_styles" | "styles") {
                    if let Some(styles) = value.as_object_mut() {
                        for style in styles.values_mut() {
                            *style = json!({"value": style.take()});
                        }
                    }
                } else {
                    expand(value);
                }
            }
            if object.contains_key("span") {
                object.entry("source_nodes").or_insert(json!([]));
                object.entry("children").or_insert(json!([]));
                if let Some(width) = object.get_mut("width") {
                    *width = json!({"value": width.take()});
                }
            }
            if object.contains_key("tag") {
                object.entry("source_node").or_insert(json!(""));
            }
            if object.contains_key("ordered")
                || object.contains_key("reversed")
                || object.contains_key("start")
                || object.contains_key("custom_style")
            {
                object.entry("ordered").or_insert(json!(false));
                object.entry("reversed").or_insert(json!(false));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_documents_roundtrip_without_diagnostics() {
        for entry in std::fs::read_dir("fixtures").unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("html") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let input = crate::input::Input::decode(bytes, path.display().to_string(), None);
            let html = scraper::Html::parse_document(&input.html);
            let Ok(selector) = crate::site::content_selector(&html, "auto") else {
                continue;
            };
            let analysis = crate::parser::parse(&html, &selector, false, None).unwrap();
            let document = from_analysis(&analysis);
            crate::snapshot::validate(&document).unwrap();
            for block in &mut analysis.blocks.clone() {
                block.visit_runs_mut(&mut |run| {
                    assert!(
                        !run.source_nodes.is_empty(),
                        "{}: {:?}",
                        path.display(),
                        run.text
                    )
                });
            }
            let bytes = serde_json::to_vec(&document).unwrap();
            let restored = crate::ir_format::read(&bytes, &path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(from_analysis(&restored), document, "{}", path.display());
            assert_eq!(
                crate::renderer::render(&analysis),
                crate::renderer::render(&restored),
                "{}",
                path.display()
            );
            assert!(restored.warnings.is_empty());
            assert_eq!(restored.original_nodes, analysis.original_nodes);
        }
    }
}
