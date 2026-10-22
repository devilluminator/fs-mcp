use std::fmt::Write;

use tokio::fs;

use crate::{
    approval,
    config::Config,
    error::{Result, ToolError},
    text::truncate,
    workspace::Workspace,
};

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_LINES: usize = 2000;
const MAX_ENTRIES: usize = 500;
const TRASH_DIR: &str = ".fs-mcp-trash";

pub async fn create_dir(ws: &Workspace, path: &str) -> Result<String> {
    let p = ws.resolve(path)?;
    fs::create_dir_all(&p).await?;
    Ok(format!("Created directory {}", p.display()))
}

pub async fn read_file(
    ws: &Workspace,
    cfg: &Config,
    path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String> {
    let p = ws.resolve(path)?;
    let meta = fs::metadata(&p).await?;
    if meta.is_dir() {
        return Err(ToolError::msg(format!("'{path}' is a directory, use list_dir")));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(ToolError::msg("file is larger than 10 MB"));
    }
    let bytes = fs::read(&p).await?;
    let text = String::from_utf8(bytes).map_err(|_| ToolError::msg("file is not valid UTF-8 text"))?;

    let start = offset.unwrap_or(1).max(1);
    let limit = limit.unwrap_or(MAX_LINES).min(MAX_LINES);
    let total = text.lines().count();

    let mut out = String::new();
    let mut last = start.saturating_sub(1);
    for (i, line) in text.lines().enumerate().skip(start - 1).take(limit) {
        let _ = writeln!(out, "{:>6}\t{}", i + 1, line);
        last = i + 1;
    }
    let mut out = truncate(&out, cfg.max_read_bytes);
    if last < total {
        let _ = write!(out, "\n[showing lines {start}-{last} of {total}; pass offset={} to continue]", last + 1);
    }
    Ok(out)
}

pub async fn write_file(ws: &Workspace, path: &str, content: &str) -> Result<String> {
    let p = ws.resolve(path)?;
    if p.is_dir() {
        return Err(ToolError::msg(format!("'{path}' is a directory")));
    }
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(&p, content).await?;
    Ok(format!("Wrote {} bytes to {}", content.len(), p.display()))
}

pub async fn edit_file(
    ws: &Workspace,
    path: &str,
    old_str: &str,
    new_str: &str,
    replace_all: bool,
) -> Result<String> {
    if old_str.is_empty() {
        return Err(ToolError::msg("old_str must not be empty"));
    }
    if old_str == new_str {
        return Err(ToolError::msg("old_str and new_str are identical"));
    }
    let p = ws.resolve(path)?;
    let text = fs::read_to_string(&p).await?;
    let count = text.matches(old_str).count();
    match count {
        0 => return Err(ToolError::msg("old_str not found (it must match exactly, including whitespace)")),
        1 => {}
        n if !replace_all => {
            return Err(ToolError::msg(format!(
                "old_str matched {n} times; add more surrounding context or set replace_all=true"
            )));
        }
        _ => {}
    }
    let updated = if replace_all {
        text.replace(old_str, new_str)
    } else {
        text.replacen(old_str, new_str, 1)
    };
    fs::write(&p, updated).await?;
    Ok(format!("Edited {} ({count} replacement{})", p.display(), if count == 1 { "" } else { "s" }))
}

pub async fn list_dir(ws: &Workspace, path: Option<&str>) -> Result<String> {
    let p = ws.resolve(path.unwrap_or("."))?;
    let mut rd = fs::read_dir(&p).await?;
    let mut entries: Vec<(String, bool, u64)> = Vec::new();
    while let Some(e) = rd.next_entry().await? {
        let Ok(meta) = e.metadata().await else { continue };
        entries.push((e.file_name().to_string_lossy().into_owned(), meta.is_dir(), meta.len()));
    }
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total = entries.len();
    let mut out = format!("{} ({total} entries)\n", p.display());
    for (name, is_dir, size) in entries.into_iter().take(MAX_ENTRIES) {
        if is_dir {
            let _ = writeln!(out, "{name}/");
        } else {
            let _ = writeln!(out, "{name}  ({size} bytes)");
        }
    }
    if total > MAX_ENTRIES {
        let _ = writeln!(out, "[showing first {MAX_ENTRIES} of {total}]");
    }
    Ok(out)
}

/// Asks the human first (showing the full absolute path), then moves the target
/// into a `.fs-mcp-trash` folder next to it, so a mistaken "yes" is recoverable
/// and the move never crosses drives.
pub async fn delete_path(ws: &Workspace, path: &str) -> Result<String> {
    let p = ws.resolve(path)?;
    let Some(parent) = p.parent() else {
        return Err(ToolError::msg("refusing to delete a drive or filesystem root"));
    };
    if p.components().any(|c| c.as_os_str() == TRASH_DIR) {
        return Err(ToolError::msg("that path is inside a trash folder"));
    }
    let meta = fs::symlink_metadata(&p)
        .await
        .map_err(|_| ToolError::msg(format!("'{}' does not exist", p.display())))?;

    let desc = if meta.is_dir() {
        let dir = p.clone();
        let n = tokio::task::spawn_blocking(move || count_files(&dir)).await.unwrap_or(0);
        format!("directory containing {n} files")
    } else {
        format!("file, {} bytes", meta.len())
    };

    let question = format!(
        "Delete '{}' ({desc})? It will be moved to a {TRASH_DIR} folder next to it, not erased.",
        p.display()
    );
    if !approval::confirm(&question).await {
        return Err(ToolError::msg("The user denied the deletion. Nothing was changed."));
    }

    let trash = parent.join(TRASH_DIR);
    fs::create_dir_all(&trash).await?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dest = trash.join(format!("{stamp}-{name}"));
    fs::rename(&p, &dest).await?;
    Ok(format!("Moved '{}' to '{}'", p.display(), dest.display()))
}

fn count_files(dir: &std::path::Path) -> usize {
    ignore::WalkBuilder::new(dir)
        .standard_filters(false)
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .count()
}
