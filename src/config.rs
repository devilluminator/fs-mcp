use std::{io, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    /// OPTIONAL jail. None (the default) = the agent may use any path your user account can.
    pub jail: Option<PathBuf>,
    pub addr: SocketAddr,
    pub require_command_approval: bool,
    pub max_read_bytes: usize,
    pub max_output_chars: usize,
}

impl Config {
    /// FS_MCP_ADDR                  listen address (default: 127.0.0.1:8099)
    /// FS_MCP_AUTO_APPROVE_COMMANDS  set to 1 to skip the prompt for run_command
    /// FS_MCP_ROOT                   OPTIONAL: restrict everything to this folder
    pub fn from_env() -> io::Result<Self> {
        let jail = match std::env::var("FS_MCP_ROOT") {
            Ok(p) => Some(dunce::canonicalize(p)?),
            Err(_) => None,
        };

        let addr = std::env::var("FS_MCP_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8099".into())
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("bad FS_MCP_ADDR: {e}")))?;

        let auto = matches!(
            std::env::var("FS_MCP_AUTO_APPROVE_COMMANDS").as_deref(),
            Ok("1")
        );

        Ok(Self {
            jail,
            addr,
            require_command_approval: !auto,
            max_read_bytes: 100_000,
            max_output_chars: 20_000,
        })
    }
}
