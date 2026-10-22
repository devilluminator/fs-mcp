use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;

use crate::{command, config::Config, error::Result as ToolResult, files, search, workspace::Workspace};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadFileArgs {
    /// Absolute path (preferred) or relative to where the server was started
    pub path: String,
    /// 1-based line to start from (default 1)
    pub offset: Option<usize>,
    /// Max lines to return (default and max 2000)
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteFileArgs {
    /// Absolute path (preferred); parent folders are created
    pub path: String,
    /// Full file content (overwrites the file if it exists)
    pub content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditFileArgs {
    pub path: String,
    /// Exact text to find; must match once unless replace_all is true
    pub old_str: String,
    pub new_str: String,
    /// Replace every occurrence (default false)
    pub replace_all: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListDirArgs {
    /// Directory to list (absolute path preferred)
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    /// Regex to look for in file contents
    pub pattern: String,
    /// Directory to search in (default ".")
    pub path: Option<String>,
    /// Optional glob filter, e.g. "*.rs"
    pub glob: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DirArgs {
    /// Directory path, absolute (e.g. "E:/nodejs/test") or relative to where the server was started
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteArgs {
    /// File or directory to delete
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunCommandArgs {
    /// Command line to run (PowerShell on Windows, sh elsewhere)
    pub command: String,
    /// Working directory (absolute path preferred)
    pub cwd: Option<String>,
    /// Timeout in seconds (default 60, max 600)
    pub timeout_secs: Option<u64>,
}

#[derive(Clone)]
pub struct FsServer {
    cfg: Arc<Config>,
    ws: Workspace,
    tool_router: ToolRouter<Self>,
}

fn finish(r: ToolResult<String>) -> std::result::Result<CallToolResult, McpError> {
    // Tool failures go back to the model as is_error results, not protocol errors,
    // so the agent can read the message and adapt.
    Ok(match r {
        Ok(s) => CallToolResult::success(vec![Content::text(s)]),
        Err(e) => CallToolResult::error(vec![Content::text(e.to_string())]),
    })
}

#[tool_router]
impl FsServer {
    pub fn new(cfg: Arc<Config>) -> Self {
        let ws = Workspace::new(cfg.jail.clone());
        Self { cfg, ws, tool_router: Self::tool_router() }
    }

    #[tool(description = "Read a text file with line numbers. Use offset/limit for large files.")]
    async fn read_file(&self, Parameters(a): Parameters<ReadFileArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(files::read_file(&self.ws, &self.cfg, &a.path, a.offset, a.limit).await)
    }

    #[tool(description = "Create or overwrite a file with the given content.")]
    async fn write_file(&self, Parameters(a): Parameters<WriteFileArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(files::write_file(&self.ws, &a.path, &a.content).await)
    }

    #[tool(description = "Replace exact text in a file. Fails if old_str is missing or matches more than once (unless replace_all).")]
    async fn edit_file(&self, Parameters(a): Parameters<EditFileArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(files::edit_file(&self.ws, &a.path, &a.old_str, &a.new_str, a.replace_all.unwrap_or(false)).await)
    }

    #[tool(description = "Create a directory (and any missing parents), like mkdir -p.")]
    async fn create_dir(&self, Parameters(a): Parameters<DirArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(files::create_dir(&self.ws, &a.path).await)
    }

    #[tool(description = "List files and folders in a directory.")]
    async fn list_dir(&self, Parameters(a): Parameters<ListDirArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(files::list_dir(&self.ws, a.path.as_deref()).await)
    }

    #[tool(description = "Regex search across file contents. Respects .gitignore.")]
    async fn search_files(&self, Parameters(a): Parameters<SearchArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(search::search_files(&self.ws, &a.pattern, a.path.as_deref(), a.glob.as_deref()).await)
    }

    #[tool(description = "Delete a file or directory. The user is asked to approve every deletion; the item is moved to a .fs-mcp-trash folder next to it.")]
    async fn delete_path(&self, Parameters(a): Parameters<DeleteArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(files::delete_path(&self.ws, &a.path).await)
    }

    #[tool(description = "Run a shell command (builds, tests, git, package managers). Not for file edits; use the file tools for those. The user may be asked to approve it.")]
    async fn run_command(&self, Parameters(a): Parameters<RunCommandArgs>) -> std::result::Result<CallToolResult, McpError> {
        finish(command::run_command(&self.ws, &self.cfg, &a.command, a.cwd.as_deref(), a.timeout_secs).await)
    }
}

#[tool_handler]
impl ServerHandler for FsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "File tools plus a command runner. Use absolute paths (e.g. E:/nodejs/test/hello.txt); they work in any directory. \
                 Prefer edit_file over rewriting whole files. Deletions and commands may be denied by the user."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
