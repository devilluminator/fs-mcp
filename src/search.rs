use std::fmt::Write;

use ignore::{WalkBuilder, overrides::OverrideBuilder};
use regex::Regex;

use crate::{
    error::{Result, ToolError},
    workspace::Workspace,
};

const MAX_MATCHES: usize = 200;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Regex search. Respects .gitignore and skips hidden files.
pub async fn search_files(
    ws: &Workspace,
    pattern: &str,
    path: Option<&str>,
    glob: Option<&str>,
) -> Result<String> {
    let re = Regex::new(pattern).map_err(|e| ToolError::msg(format!("invalid regex: {e}")))?;
    let base = ws.resolve(path.unwrap_or("."))?;
    let glob = glob.map(str::to_owned);
    let ws = ws.clone();

    tokio::task::spawn_blocking(move || {
        let mut walker = WalkBuilder::new(&base);
        if let Some(g) = glob {
            let mut ob = OverrideBuilder::new(&base);
            ob.add(&g).map_err(|e| ToolError::msg(format!("invalid glob: {e}")))?;
            let ov = ob.build().map_err(|e| ToolError::msg(format!("invalid glob: {e}")))?;
            walker.overrides(ov);
        }

        let mut out = String::new();
        let mut hits = 0usize;
        'files: for entry in walker.build().flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if entry.metadata().map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue; // binary or non-UTF-8
            };
            let shown_path = ws.display(entry.path());
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    let shown: String = line.trim().chars().take(200).collect();
                    let _ = writeln!(out, "{shown_path}:{}: {shown}", i + 1);
                    hits += 1;
                    if hits >= MAX_MATCHES {
                        let _ = writeln!(out, "[stopped at {MAX_MATCHES} matches, narrow the search]");
                        break 'files;
                    }
                }
            }
        }
        if hits == 0 {
            out.push_str("No matches.");
        }
        Ok(out)
    })
    .await
    .map_err(|e| ToolError::msg(format!("search task failed: {e}")))?
}
