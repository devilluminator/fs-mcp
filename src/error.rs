#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("path '{0}' is outside the workspace")]
    Outside(String),
    #[error("{0}")]
    Msg(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ToolError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Msg(s.into())
    }
}

pub type Result<T> = std::result::Result<T, ToolError>;
