// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! `noyalib-mcp` — a Model Context Protocol server for lossless YAML
//! editing.
//!
//! Six tools, one prompt and three resources. The protocol is handled
//! by [`rmcp`], the official MCP SDK; this crate supplies the tools and
//! the text a model reads. The `noyalib-mcp` binary serves the same
//! handler over stdio, streamable HTTP or the older HTTP+SSE transport
//! (see `src/transport.rs`, one file shared with every Rust server of
//! the suite).
//!
//! # Why this exists
//!
//! AI agents that edit YAML configuration regex-replace and corrupt
//! comments and formatting. noyalib's CST does the edits losslessly:
//! `noyalib_set` rewrites only the byte span of the value it changes,
//! so every comment, blank line and sibling entry survives. This
//! server is what lets Claude, Cursor, Zed and any other MCP-aware
//! client drive that engine safely.
//!
//! # The tools
//!
//! - `noyalib_get` reads the value at a dotted/indexed path in a file.
//! - `noyalib_set` sets it, preserving every untouched byte.
//! - `noyalib_set_multidoc` is `noyalib_set` for one document of a
//!   `---`-separated stream.
//! - `noyalib_parse`, `noyalib_edit` and `noyalib_validate` work on
//!   YAML text given in the request and touch nothing on disk.
//!
//! Each is a plain function in [`tools`] -- [`get`], [`set`],
//! [`set_multidoc`], [`parse`], [`edit`], [`validate`] -- and
//! [`YamlServer`] is the handler that exposes them as MCP tools. Every
//! answer is returned twice: as text for the model, and as
//! `structuredContent` against the tool's `outputSchema` for a client
//! that wants to read it without parsing prose.
//!
//! # Errors
//!
//! A tool that ran and could not do the job -- a file that cannot be
//! read, YAML that does not parse, a path that does not exist -- is a
//! *successful* JSON-RPC response carrying `isError: true` and text the
//! model can act on. A tool the server does not have is reported the
//! same way, naming the tools that exist. A request the protocol
//! rejects (an unknown method, a resource that does not exist) is a
//! JSON-RPC error, which the client handles.
//!
//! # Install + connect
//!
//! ```text
//! cargo install noyalib-mcp
//! claude mcp add noyalib -- noyalib-mcp
//! ```
//!
//! # MSRV
//!
//! **Rust 1.88.0** stable, the floor of the SDK. The manifest's
//! `rust-version` is the contract and CI verifies it.
//!
//! # Panics
//!
//! Public functions in this crate do not panic on any input; a fuzz
//! target feeds the stateless tools arbitrary bytes to keep it so.
//!
//! # Security
//!
//! `#![forbid(unsafe_code)]`. No FFI. The file tools read and write
//! whatever the process may; confine a deployment with container
//! mounts or systemd `ReadWritePaths=`. The HTTP listeners exist only
//! when asked for on the command line, bind loopback by default and do
//! not authenticate. Resource-limit gates are inherited from
//! `noyalib`'s `ParserConfig` defaults. Full posture:
//! [`SECURITY.md`](https://github.com/sebastienrousseau/noyalib-mcp/blob/main/SECURITY.md).
//!
//! # API stability and SemVer
//!
//! Pre-1.0 (`0.0.x`): the MCP wire contract (tool names, input-schema
//! shapes, the failure taxonomy) is **stable** within a 0.0.x line --
//! bug fixes only. Adding a tool is allowed within a 0.0.x bump;
//! removing or renaming one is held to a 0.x bump. The Rust surface is
//! the six functions, their argument and output types, and
//! [`YamlServer`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, ListResourceTemplatesResult, ListResourcesResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, ServerCapabilities,
    ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, prompt_handler, tool_handler};

pub mod prompts;
pub mod resources;
pub mod tools;

pub use tools::{
    EditArgs, EditOutput, GetArgs, GetOutput, ParseArgs, ParseOutput, SetArgs, SetMultidocArgs,
    SetMultidocOutput, SetOutput, TOOL_NAMES, ValidateArgs, ValidateOutput, Violation, edit, get,
    parse, set, set_multidoc, validate,
};

/// One hour, in milliseconds: the freshness hint on the cacheable
/// results. The catalogue is fixed per binary, so any bound would do;
/// an hour keeps re-listing cheap without letting a stale cache
/// survive a server upgrade for long.
const CACHE_TTL_MS: u64 = 3_600_000;

/// What `server/discover` and `initialize` say the server is for.
const INSTRUCTIONS: &str = "Read and edit YAML files losslessly. \
    noyalib_get reads the value at a dotted/indexed path in a file; \
    noyalib_set and noyalib_set_multidoc rewrite one value while \
    preserving every comment and all formatting. noyalib_parse, \
    noyalib_edit and noyalib_validate do the same for YAML text given \
    in the request and touch nothing on disk.";

/// The MCP server: the six tools, the prompt and the resources over
/// [`rmcp`].
///
/// Cheap to create and to clone; the HTTP transports create one per
/// session. It holds no document between calls.
#[derive(Debug, Clone)]
pub struct YamlServer {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl Default for YamlServer {
    fn default() -> Self {
        Self::new()
    }
}

impl YamlServer {
    /// A server with every tool and prompt registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for YamlServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
        )
        .with_server_info(
            Implementation::new("noyalib-mcp", env!("CARGO_PKG_VERSION"))
                .with_title("Noyalib MCP")
                .with_website_url(env!("CARGO_PKG_REPOSITORY")),
        )
        .with_instructions(INSTRUCTIONS)
    }

    /// A tool the server does not have is reported as a tool result,
    /// not a protocol error.
    ///
    /// The SDK's default is `-32602`, which the stateless HTTP revision
    /// carries as an HTTP 400 -- a transport fault to the client, and
    /// nothing a model gets to read. A model that misspelt a tool name
    /// is better served by text saying so.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !self.tool_router.has_route(&request.name) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Unknown tool: {}. The tools are {}.",
                request.name,
                TOOL_NAMES.join(", ")
            ))])
            .into());
        }
        let call = ToolCallContext::new(self, request, context);
        self.tool_router.call(call).await
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(resources::list()).with_ttl_ms(CACHE_TTL_MS))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(
            ListResourceTemplatesResult::with_all_items(resources::templates())
                .with_ttl_ms(CACHE_TTL_MS),
        )
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let contents = resources::read(&request.uri, &self.tool_router.list_all())?;
        Ok(ReadResourceResult::new(vec![contents])
            .with_ttl_ms(CACHE_TTL_MS)
            .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_describes_itself() {
        let info = YamlServer::default().get_info();
        assert_eq!(info.server_info.name, "noyalib-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.prompts.is_some());
        assert!(info.capabilities.resources.is_some());
        assert!(
            info.instructions
                .as_deref()
                .is_some_and(|i| i.contains("noyalib_get") && i.contains("noyalib_validate"))
        );
    }

    #[test]
    fn the_routers_agree_with_the_name_lists() {
        let s = YamlServer::new();
        for name in TOOL_NAMES {
            assert!(s.tool_router.has_route(name), "{name}");
        }
        assert!(!s.tool_router.has_route("frobnicate"));
        for name in prompts::PROMPT_NAMES {
            assert!(s.prompt_router.has_route(name), "{name}");
        }
    }
}
