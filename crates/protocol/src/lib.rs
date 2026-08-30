mod commands;
mod events;
mod ids;
mod inspect_url;
mod state;
mod types;

pub use commands::DebuggerCommand;
pub use events::{BrowserEvent, EventKind, LogEvent};
pub use ids::{ExecutionId, PageId};
pub use inspect_url::{
    find_page_for_url, normalize_inspect_url, origin_from_http_url, page_matches_inspect_url,
};
pub use state::{ConnectionStatus, DebuggerState};
pub use types::{Page, Tool, ToolAnnotations, ToolExecution};

pub const PROTOCOL_VERSION: u32 = 1;
pub const WS_HOST: &str = "127.0.0.1";
pub const WS_PORT: u16 = 17321;
