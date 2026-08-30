use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageId(pub String);

impl PageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PageId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ExecutionId(pub String);

impl ExecutionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ExecutionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
