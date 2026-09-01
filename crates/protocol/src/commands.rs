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
    /// Sent on a timer purely so the MV3 service worker stays alive.
    ///
    /// Chrome evicts an idle extension service worker after about 30 seconds.
    /// Handling a message resets that timer, so a periodic no-op is what keeps
    /// the bridge up. Without it the extension dies, the socket drops, and the
    /// debugger sees no tools at all until something happens to revive it.
    Ping,
    /// Ask the page to abort a run. WebMCP passes an `AbortSignal` as the second
    /// argument to `execute`, so this is a real abort rather than us just looking
    /// away — where the browser supports it.
    CancelExecution {
        page_id: PageId,
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
