//! Adding, changing and deleting agentZ's MCP servers ([`crate::mcp_servers`]).

use agentz_protocol::{Request, Response};
use anyhow::{Result, anyhow};

use super::Server;
use crate::mcp_servers;

impl Server {
    pub(super) fn mcp_server_request(&mut self, request: Request) -> Result<Response> {
        let data_dir = self.data_dir.clone();
        match request {
            Request::SaveMcpServer { replacing, server } => mcp_servers::save_server(
                &data_dir,
                &mut self.mcp_servers,
                replacing.as_deref(),
                server,
            )?,
            Request::DeleteMcpServer(name) => {
                mcp_servers::delete_server(&data_dir, &mut self.mcp_servers, &name)?
            }
            Request::SetMcpServerEnabled { name, enabled } => {
                mcp_servers::set_enabled(&data_dir, &mut self.mcp_servers, &name, enabled)?
            }
            request => return Err(anyhow!("not an MCP server request: {request:?}")),
        }
        Ok(Response::Ok)
    }
}
