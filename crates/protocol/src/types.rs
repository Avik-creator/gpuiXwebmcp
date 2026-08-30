use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{ExecutionId, PageId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub id: PageId,
    pub url: String,
    pub title: String,
    pub origin: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub untrusted_content_hint: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    pub input_schema: Value,
    #[serde(default)]
    pub annotations: ToolAnnotations,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecution {
    pub id: ExecutionId,
    pub tool_name: String,
    pub arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}
