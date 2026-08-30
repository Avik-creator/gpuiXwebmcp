use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{ExecutionId, PageId};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DebuggerCommand {
    SubscribePage {
        page_id: PageId,
    },
    ExecuteTool {
        page_id: PageId,
        tool: String,
        arguments: Value,
        execution_id: ExecutionId,
    },
}
