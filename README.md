# fs-mcp

A small [MCP](https://modelcontextprotocol.io) server written in Rust that gives an AI agent **separate file tools** (read, write, edit, search, delete) plus a **command runner**, served over Streamable HTTP. Built for local setups such as LM Studio.

Instead of one "run any shell command" tool, the agent gets structured, typed file operations that are easier for models (especially smaller local ones) to call correctly, and that can be gated individually. Destructive actions stay behind a human approval prompt.

## Features

- **Structured file tools**: no shell quoting or escaping problems, and the same behavior on Windows and Linux.
- **Exact-match editing**: `edit_file` fails with a clear message when the text is missing or ambiguous, so the model can't silently clobber the wrong place.
- **Human approval for deletes**: `delete_path` blocks inside the tool until you answer `y` in the server terminal. The default is **No**. The model cannot skip it.
- **Recoverable deletes**: approved deletions are moved into a `.fs-mcp-trash` folder next to the original instead of being erased.
- **Optional approval for commands**: `run_command` asks before running unless you opt out.
- **Works on any path**: absolute paths such as `E:/nodejs/test/hello.txt` work anywhere your user account can reach, including other drives.
- **Optional jail**: set `FS_MCP_ROOT` to confine everything to one folder.
- **Bounded output**: read limits, match caps, command timeouts, and output truncation keep the model's context from flooding.

## Tools

| Tool | What it does |
| --- | --- |
| `read_file` | Read a text file with line numbers. Supports `offset` and `limit`. |
| `write_file` | Create or overwrite a file. Missing parent folders are created. |
| `edit_file` | Replace exact text. Fails if `old_str` is missing, or matches more than once unless `replace_all` is set. |
| `create_dir` | Create a directory and any missing parents (`mkdir -p`). |
| `list_dir` | List files and folders in a directory. |
| `search_files` | Regex search across file contents. Respects `.gitignore`, optional glob filter. |
| `delete_path` | Delete a file or directory **after you approve it**. Moves the item to `.fs-mcp-trash`. |
| `run_command` | Run a command (PowerShell on Windows, `sh` elsewhere) with a timeout and output cap. Asks for approval by default. |

Paths should be absolute. Relative paths resolve against the folder the server was started in.

## Requirements

- Rust 1.85 or newer (the crate uses the 2024 edition)

## Quick start

```bash
git clone https://github.com/<your-user>/fs-mcp.git
cd fs-mcp
cargo run --release
```

The server listens on `http://127.0.0.1:8099/mcp`.

Keep the terminal window visible. Approval prompts for deletes and commands appear **there**, not in your MCP client.

## Connect it to LM Studio

Add the server to your `mcp.json`:

```json
{
  "mcpServers": {
    "fs-mcp": {
      "url": "http://127.0.0.1:8099/mcp"
    }
  }
}
```

Any other MCP client that supports Streamable HTTP can use the same URL.

## Configuration

All settings are optional environment variables.

| Variable | Default | Description |
| --- | --- | --- |
| `FS_MCP_ADDR` | `127.0.0.1:8099` | Address and port to listen on. |
| `FS_MCP_AUTO_APPROVE_COMMANDS` | unset | Set to `1` to skip the approval prompt for `run_command`. Only do this in a throwaway workspace. |
| `FS_MCP_ROOT` | unset | Restrict all file tools to this folder (`..` and symlink escapes are rejected). |

PowerShell example:

```powershell
$env:FS_MCP_ADDR = "127.0.0.1:8100"
cargo run --release
```

## How approval works

Approval is enforced **inside the tool**, not by a prompt that asks the model to behave.

1. The agent calls `delete_path("E:/nodejs/test")`.
2. The server prints the full absolute path, whether it is a file or directory, and how many files a directory contains, then waits:

   ```
   [approval needed] Delete 'E:\nodejs\test' (directory containing 3 files)? It will be moved to a .fs-mcp-trash folder next to it, not erased.
   Allow? [y/N]
   ```

3. `y` or `yes` runs it. Anything else, including an empty line or a closed stdin, denies it, and the agent receives "The user denied the deletion" so it can adapt.

`delete_path` also refuses to delete a drive or filesystem root, or anything already inside a trash folder.

## Safety notes

- **File tools are not sandboxed by default.** With no `FS_MCP_ROOT`, the agent can read and overwrite anything your user account can, including other projects and dotfiles. Only deletes and commands prompt you. Set `FS_MCP_ROOT` if you want a boundary.
- **`run_command` is not confined by the jail.** A shell command can reach anywhere your user can, because a command string can't be reliably inspected. The approval prompt is the protection, so read the command before you type `y`. For a real boundary, run the server inside Docker, WSL, or a VM with only your project mounted.
- **Each `run_command` call is a fresh process.** A `cd` inside one command does not carry over to the next call.
- **Keep it on localhost.** The server has no authentication. Don't bind it to a public interface.

## Project layout

```
src/
  main.rs       Streamable HTTP server setup
  lib.rs        module list
  server.rs     tool definitions and argument schemas
  files.rs      read / write / edit / list / create_dir / delete
  search.rs     regex search
  command.rs    command runner
  approval.rs   terminal y/N prompt
  workspace.rs  path resolution and optional jail
  config.rs     environment variable settings
  error.rs      error type
  text.rs       output truncation helper
```
