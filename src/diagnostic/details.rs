//! 诊断详情的字段约定。正文值可为文字、数值或对象，其余字段使用具体类型。
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Details {
    #[serde(skip_serializing_if = "Value::is_null")]
    pub original: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub value: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub evidence: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub expected: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub actual: Value,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub network: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconstructed_excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<String>,
    pub ir_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inferred: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_difference: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub source_nodes: Vec<String>,
}
