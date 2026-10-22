use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

use crate::error::{Result, ToolError};

/// Stateless path resolver. Absolute paths (e.g. "E:/nodejs/test") work anywhere;
/// relative paths resolve against the folder the server process was started in.
/// An optional jail (FS_MCP_ROOT) restricts everything to one folder.
#[derive(Clone, Debug)]
pub struct Workspace {
    jail: Option<PathBuf>, // canonicalized
}

impl Workspace {
    pub fn new(jail: Option<PathBuf>) -> Self {
        Self { jail }
    }

    /// Base for relative paths: the server process's own working directory.
    pub fn base(&self) -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    pub fn resolve(&self, input: &str) -> Result<PathBuf> {
        // join() replaces the base when `input` is absolute
        let joined = self.base().join(input);

        let mut normalized = PathBuf::new();
        for c in joined.components() {
            match c {
                Component::ParentDir => {
                    normalized.pop(); // at a filesystem root, ".." stays put like the OS does
                }
                Component::CurDir => {}
                other => normalized.push(other.as_os_str()),
            }
        }

        let Some(jail) = &self.jail else {
            return Ok(normalized);
        };

        // Jail mode: canonicalize the deepest existing ancestor (follows symlinks),
        // check it, then re-append the not-yet-existing tail.
        let outside = || ToolError::Outside(input.into());
        let mut anchor = normalized;
        let mut tail: Vec<OsString> = Vec::new();
        while !anchor.exists() {
            let name = anchor.file_name().map(|n| n.to_owned()).ok_or_else(outside)?;
            tail.push(name);
            if !anchor.pop() {
                return Err(outside());
            }
        }
        let mut resolved = dunce::canonicalize(&anchor)?;
        if !resolved.starts_with(jail) {
            return Err(outside());
        }
        for name in tail.into_iter().rev() {
            resolved.push(name);
        }
        Ok(resolved)
    }

    /// Relative to the base when inside it, otherwise absolute.
    pub fn display(&self, p: &Path) -> String {
        match p.strip_prefix(self.base()) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".into(),
            Ok(rel) => rel.display().to_string(),
            Err(_) => p.display().to_string(),
        }
    }
}
