use std::io::Write;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::Mutex,
};

// One prompt at a time so concurrent tool calls don't interleave.
static PROMPT_LOCK: Mutex<()> = Mutex::const_new(());

/// Blocks until the human answers in the server terminal.
/// Default is NO: empty input, EOF, or a read error all deny.
/// This runs inside the tool, so the model cannot skip it.
pub async fn confirm(question: &str) -> bool {
    let _guard = PROMPT_LOCK.lock().await;
    eprintln!("\n[approval needed] {question}");
    eprint!("Allow? [y/N] ");
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    let mut reader = BufReader::new(tokio::io::stdin());
    match reader.read_line(&mut line).await {
        Ok(n) if n > 0 => matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"),
        _ => false,
    }
}
