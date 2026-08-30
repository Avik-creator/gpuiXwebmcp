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
    /// User-typed http(s) URL from the debugger search bar. Never a page payload.
    OpenPage {
        url: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_page_uses_snake_case_type_tag() {
        let json = serde_json::to_value(DebuggerCommand::OpenPage {
            url: "http://localhost:5173/".into(),
        })
        .unwrap();
        assert_eq!(json["type"], "open_page");
        assert_eq!(json["url"], "http://localhost:5173/");
    }
}
