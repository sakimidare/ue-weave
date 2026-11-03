//! Versioned editing contract and contextual input validation.
use crate::{
    error::{RecoverError, Result},
    model::{Block, Ir},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub version: u32,
}
impl Default for Schema {
    fn default() -> Self {
        Self {
            name: "ue-weave-ir".into(),
            version: 1,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub version: String,
}
impl Default for Tool {
    fn default() -> Self {
        Self {
            name: "ue-weave".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}
fn invalid(field: &str, reason: &str) -> RecoverError {
    RecoverError::InvalidIr {
        field: field.into(),
        reason: reason.into(),
    }
}

pub fn read(bytes: &[u8], path: &Path) -> Result<Ir> {
    let value: Value = serde_json::from_slice(bytes).map_err(|source| RecoverError::ReadIr {
        path: path.into(),
        source,
    })?;
    value
        .as_object()
        .ok_or_else(|| invalid("$", "需要 JSON 对象"))?;
    let schema = &value["schema"];
    if schema["name"] != "ue-weave-ir" {
        return Err(invalid("schema.name", "需要 ue-weave-ir"));
    }
    if schema["version"] != 1 {
        return Err(invalid(
            "schema.version",
            "当前支持版本 1；请使用支持该版本的工具",
        ));
    }
    // Deserialize each node separately so errors identify the edited JSON path.
    validate_blocks(
        value
            .get("blocks")
            .ok_or_else(|| invalid("blocks", "缺少正文块数组"))?,
        "blocks",
        0,
    )?;
    let mut ir: Ir = serde_json::from_value(value).map_err(|source| RecoverError::ReadIr {
        path: path.into(),
        source,
    })?;
    normalize(&mut ir.blocks);
    ir.tool = Tool::default();
    Ok(ir)
}

fn validate_blocks(value: &Value, path: &str, depth: usize) -> Result<()> {
    if depth > 64 {
        return Err(invalid(path, "正文嵌套深度超过 64"));
    }
    let blocks = value.as_array().ok_or_else(|| invalid(path, "需要数组"))?;
    for (i, value) in blocks.iter().enumerate() {
        let path = format!("{path}[{i}]");
        let kind = value["type"]
            .as_str()
            .ok_or_else(|| invalid(&format!("{path}.type"), "需要节点类型"))?;
        if ![
            "paragraph",
            "heading",
            "signature",
            "list",
            "list_item",
            "table",
            "quote",
            "code_block",
            "horizontal_rule",
        ]
        .contains(&kind)
        {
            return Err(invalid(&format!("{path}.type"), "未知节点类型"));
        }
        if matches!(kind, "paragraph" | "heading" | "signature")
            && value.get("runs").is_none()
            && value["text"].as_str().is_some_and(|text| !text.is_empty())
        {
            return Err(invalid(
                &format!("{path}.runs"),
                "普通正文通过 runs 保存文本；请提供文本片段数组",
            ));
        }
        if let Some(runs) = value.get("runs") {
            validate_runs(runs, &format!("{path}.runs"))?;
        }
        if let Some(runs) = value.get("caption").filter(|v| !v.is_null()) {
            validate_runs(runs, &format!("{path}.caption"))?;
        }
        if let Some(children) = value.get("children") {
            validate_blocks(children, &format!("{path}.children"), depth + 1)?;
        }
        if let Some(rows) = value.get("rows") {
            let rows = rows
                .as_array()
                .ok_or_else(|| invalid(&format!("{path}.rows"), "需要数组"))?;
            for (r, row) in rows.iter().enumerate() {
                let row_path = format!("{path}.rows[{r}]");
                for (c, cell) in row
                    .as_array()
                    .ok_or_else(|| invalid(&row_path, "需要单元格数组"))?
                    .iter()
                    .enumerate()
                {
                    let cell_path = format!("{row_path}[{c}]");
                    if let Some(runs) = cell.get("runs") {
                        validate_runs(runs, &format!("{cell_path}.runs"))?;
                    }
                    for key in ["colspan", "rowspan"] {
                        if let Some(span) = cell.get(key) {
                            if !span.as_u64().is_some_and(|n| (1..=1000).contains(&n)) {
                                return Err(invalid(
                                    &format!("{cell_path}.{key}"),
                                    "需要 1 到 1000 的整数",
                                ));
                            }
                        }
                    }
                    if let Some(children) = cell.get("blocks") {
                        validate_blocks(children, &format!("{cell_path}.blocks"), depth + 1)?;
                    }
                }
            }
        }
        if let Some(level) = value.get("level").filter(|v| !v.is_null()) {
            if !level["value"]
                .as_u64()
                .is_some_and(|n| (1..=6).contains(&n))
            {
                return Err(invalid(
                    &format!("{path}.level.value"),
                    "需要 1 到 6 的整数",
                ));
            }
        }
        serde_json::from_value::<Block>(value.clone())
            .map_err(|e| invalid(&path, &e.to_string()))?;
    }
    Ok(())
}

fn validate_runs(value: &Value, path: &str) -> Result<()> {
    let runs = value
        .as_array()
        .ok_or_else(|| invalid(path, "需要文本片段数组"))?;
    for (index, run) in runs.iter().enumerate() {
        for (key, image) in [("link", false), ("image", true)] {
            if let Some(url) = run.get(key).filter(|v| !v.is_null()) {
                if !url
                    .as_str()
                    .is_some_and(|url| crate::safety::safe_url(url, image))
                {
                    return Err(invalid(
                        &format!("{path}[{index}].{key}"),
                        "请使用 HTTP/HTTPS 地址或资源相对路径；文字链接也支持 mailto/tel",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn normalize(blocks: &mut [Block]) {
    for block in blocks {
        // Runs are the editable text source; code blocks keep their literal text.
        if block.kind != "code_block" && !block.runs.is_empty() {
            block.text = block.runs.iter().map(|run| run.text.as_str()).collect();
        }
        normalize(&mut block.children);
        for cell in block.rows.iter_mut().flatten() {
            normalize(&mut cell.blocks);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn requires_explicit_schema() {
        assert!(read(br#"{"blocks":[]}"#, Path::new("edited.json"))
            .err()
            .unwrap()
            .to_string()
            .contains("schema.name"));
    }
    fn load(mut value: Value) -> Result<Ir> {
        if value.get("schema").is_none() {
            value["schema"] = json!(Schema::default());
        }
        value["tool"] = json!(Tool::default());
        read(
            &serde_json::to_vec(&value).unwrap(),
            Path::new("edited.json"),
        )
    }
    #[test]
    fn editing_defaults_and_version_roundtrip() {
        let ir =
            load(json!({"blocks":[{"type":"paragraph","text":"old","runs":[{"text":"edited"}]}]}))
                .unwrap();
        assert_eq!(ir.schema.version, 1);
        assert_eq!(ir.blocks[0].text, "edited");
        assert!(!ir.blocks[0].runs[0].bold);
        assert_eq!(ir.blocks[0].align, "unknown");
        let roundtrip = load(json!(ir)).unwrap();
        assert!(roundtrip.warnings.is_empty());
    }
    #[test]
    fn rejects_future_versions_and_reports_nested_errors() {
        assert!(
            load(json!({"schema":{"name":"ue-weave-ir","version":2},"blocks":[]}))
                .err()
                .unwrap()
                .to_string()
                .contains("schema.version")
        );
        for (value, expected) in [
            (json!({"blocks":[{"type":"mystery"}]}), "blocks[0].type"),
            (
                json!({"blocks":[{"type":"table","rows":[[{"colspan":0}]]}]}),
                "blocks[0].rows[0][0].colspan",
            ),
            (
                json!({"blocks":[{"type":"heading","level":{"value":8}}]}),
                "blocks[0].level.value",
            ),
        ] {
            assert!(load(value).err().unwrap().to_string().contains(expected));
        }
    }
}
