// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! One command line for the three MCP transports.
//!
//! Every server in the suite is started the same way:
//!
//! ```text
//! <server>                                   # stdio, for a client that spawns it
//! <server> --transport streamable-http       # HTTP on 127.0.0.1:8000, path /mcp
//! <server> --transport sse --port 8001       # the older HTTP+SSE transport
//! ```
//!
//! Over streamable HTTP the SDK speaks both current protocol revisions
//! on one endpoint: `2026-07-28` (stateless, `server/discover`,
//! per-request `_meta`) and `2025-11-25` (`initialize` handshake,
//! `Mcp-Session-Id`). Responses stream as server-sent events; a `GET`
//! on the same path opens the server-to-client event stream.
//! `--transport sse` serves the `2024-11-05` HTTP+SSE transport for
//! clients that still expect it: `GET /sse` opens the event stream,
//! whose first event names the `/messages/?sessionId=...` endpoint the
//! client posts to.
//!
//! The listener binds the loopback interface unless told otherwise.
//! There is no authentication here: put the server behind a gateway
//! you trust before binding a routable address.
//!
//! This module (`transport.rs` and `transport/`) depends only on the
//! SDK and on a [`ServerHandler`] passed in, so it is copied verbatim
//! into every Rust server of the suite. Nothing in it knows what the
//! tools are.

use std::io;
use std::path::{Path, PathBuf};

use noyalib_mcp::ParseProfile;
use std::process::ExitCode;

use axum::Router;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, ServiceExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

/// Where the listeners bind unless told otherwise.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// The port the HTTP transports listen on unless told otherwise.
pub const DEFAULT_PORT: u16 = 8000;
/// How many sessions an HTTP transport holds at once unless told
/// otherwise.
pub const DEFAULT_MAX_SESSIONS: usize = 64;
/// The streamable HTTP endpoint.
pub const STREAMABLE_HTTP_PATH: &str = "/mcp";
/// Where the legacy transport's event stream is opened.
pub const SSE_PATH: &str = "/sse";
/// Where the legacy transport's client messages are posted.
pub const MESSAGE_PATH: &str = "/messages/";

/// How the server talks to its client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// JSON-RPC over the process's own stdin and stdout.
    Stdio,
    /// The current HTTP transport, both protocol revisions.
    StreamableHttp,
    /// The 2024-11-05 HTTP+SSE transport.
    Sse,
}

impl Transport {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "stdio" => Some(Self::Stdio),
            "streamable-http" => Some(Self::StreamableHttp),
            "sse" => Some(Self::Sse),
            _ => None,
        }
    }
}

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The transport to serve.
    pub transport: Transport,
    /// The interface the HTTP transports bind.
    pub host: String,
    /// The port the HTTP transports bind. Zero asks the system for a
    /// free one, which is what a test wants.
    pub port: u16,
    /// The directory the file tools are confined to; `None` means the
    /// working directory.
    pub root: Option<PathBuf>,
    /// The rules the parse tools apply.
    pub profile: ParseProfile,
    /// How many sessions an HTTP transport holds at once; past it, a
    /// new one is refused.
    pub max_sessions: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            transport: Transport::Stdio,
            host: DEFAULT_HOST.to_owned(),
            port: DEFAULT_PORT,
            root: None,
            profile: ParseProfile::default(),
            max_sessions: DEFAULT_MAX_SESSIONS,
        }
    }
}

/// The outcome of reading the command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Serve with these options.
    Serve(Options),
    /// `--help`: print the usage text and exit.
    Help,
    /// `--version`: print the version and exit.
    Version,
}

/// The usage text, for `--help` and for a usage error.
#[must_use]
pub fn usage(name: &str) -> String {
    format!(
        "Usage: {name} [--transport <stdio|streamable-http|sse>] \
         [--host <address>] [--port <number>] [--root <dir>] [--profile <strict|standard>] \
         [--max-sessions <n>]\n\
         \n\
         Options:\n\
         \x20 --transport <name>  stdio (default), streamable-http, or sse\n\
         \x20 --host <address>    interface for the HTTP transports \
         (default {DEFAULT_HOST})\n\
         \x20 --port <number>     port for the HTTP transports \
         (default {DEFAULT_PORT})\n\
         \x20 --root <dir>        directory the file tools may read and write \
         (default: the working directory, unless that is / or the home directory)\n\
         \x20 --profile <name>    rules every tool parses under: \
         strict (default) or standard\n\
         \x20 --max-sessions <n>  sessions an HTTP transport holds at once \
         (default {DEFAULT_MAX_SESSIONS})\n\
         \x20 --version           print the version and exit\n\
         \x20 --help              print this text and exit\n\
         \n\
         streamable-http serves {STREAMABLE_HTTP_PATH} (MCP 2025-11-25 and \
         2026-07-28).\n\
         sse serves {SSE_PATH} and {MESSAGE_PATH} (MCP 2024-11-05).\n\
         Neither transport authenticates: keep them on the loopback \
         interface or\n\
         behind a gateway you trust."
    )
}

/// Read the command line.
///
/// `args` excludes the program name. Both `--flag value` and
/// `--flag=value` are accepted.
///
/// # Errors
///
/// A flag that is unknown, missing its value, or given a value that
/// does not parse. The message says which.
pub fn parse<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut options = Options::default();
    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        let (flag, inline) = split_flag(arg);
        match flag.as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            other => apply_flag(&mut options, other, inline, &mut args)?,
        }
    }
    Ok(Command::Serve(options))
}

/// Split `--flag=value` at its first `=`; a bare `--flag` has no
/// inline value.
fn split_flag(arg: String) -> (String, Option<String>) {
    match arg.split_once('=') {
        Some((flag, value)) => (flag.to_owned(), Some(value.to_owned())),
        None => (arg, None),
    }
}

/// Set the option `flag` names, from its inline value or else the next
/// argument. An unknown flag fails before any argument is read.
fn apply_flag(
    options: &mut Options,
    flag: &str,
    inline: Option<String>,
    rest: &mut dyn Iterator<Item = String>,
) -> Result<(), String> {
    if !matches!(
        flag,
        "--transport" | "--host" | "--port" | "--root" | "--profile" | "--max-sessions"
    ) {
        return Err(format!("unknown argument `{flag}`"));
    }
    let value = inline
        .or_else(|| rest.next())
        .ok_or_else(|| format!("{flag} needs a value"))?;
    set_option(options, flag, value)
}

/// Set the option a known `flag` names to `value`.
fn set_option(options: &mut Options, flag: &str, value: String) -> Result<(), String> {
    match flag {
        "--root" => options.root = Some(PathBuf::from(value)),
        "--host" => options.host = value,
        _ => set_parsed(options, flag, &value)?,
    }
    Ok(())
}

/// Set an option whose value must parse: transport, port, session
/// limit or profile.
fn set_parsed(options: &mut Options, flag: &str, value: &str) -> Result<(), String> {
    match flag {
        "--transport" => options.transport = parse_transport(value)?,
        "--port" => options.port = parse_port(value)?,
        "--max-sessions" => options.max_sessions = parse_max_sessions(value)?,
        _ => options.profile = parse_profile(value)?,
    }
    Ok(())
}

fn parse_profile(name: &str) -> Result<ParseProfile, String> {
    ParseProfile::from_name(name)
        .ok_or_else(|| format!("unknown profile `{name}`; choose strict or standard"))
}

fn parse_transport(name: &str) -> Result<Transport, String> {
    Transport::parse(name)
        .ok_or_else(|| format!("unknown transport `{name}`; choose stdio, streamable-http or sse"))
}

fn parse_max_sessions(text: &str) -> Result<usize, String> {
    text.parse()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("`{text}` is not a positive number of sessions"))
}

fn parse_port(text: &str) -> Result<u16, String> {
    text.parse()
        .map_err(|_| format!("`{text}` is not a port number"))
}

/// The directory the file tools are confined to: `--root` when given,
/// else the working directory. It must exist and be a directory. The
/// path is returned as given (made absolute), so an absolute `file`
/// argument spelt the way the operator spelt the root still matches;
/// the server also matches the canonical form.
fn resolve_root(root: Option<&Path>) -> Result<PathBuf, String> {
    let chosen = match root {
        Some(r) => r.to_path_buf(),
        // An unreadable working directory falls back to `.`, which the
        // canonicalisation below then reports with its own error.
        None => std::env::current_dir().unwrap_or(PathBuf::from(".")),
    };
    let canonical = chosen
        .canonicalize()
        .map_err(|e| format!("--root {}: {e}", chosen.display()))?;
    if canonical.is_dir() {
        Ok(std::path::absolute(&chosen).unwrap_or(canonical))
    } else {
        Err(format!("--root {}: not a directory", chosen.display()))
    }
}

/// Refuse the working directory as the default root when it is the
/// filesystem root or the user's home directory: a client that spawns
/// the server there without `--root` would hand the model every file.
/// Asked for with `--root`, either is the operator's choice.
fn refuse_broad_default(root: &Path, home: Option<&Path>) -> Result<(), String> {
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let is_home = home
        .and_then(|h| h.canonicalize().ok())
        .is_some_and(|h| h == canonical);
    if canonical.parent().is_none() || is_home {
        return Err(format!(
            "refusing to serve {} without --root: it is the filesystem root \
             or the home directory; pass --root <dir> to choose the directory \
             the file tools may use",
            root.display()
        ));
    }
    Ok(())
}

/// The root `options` ask for, checked: `--root`, or a working
/// directory that is neither `/` nor the home directory.
fn chosen_root(options: &Options) -> Result<PathBuf, String> {
    let root = resolve_root(options.root.as_deref())?;
    if options.root.is_none() {
        refuse_broad_default(&root, home_dir().as_deref())?;
    }
    Ok(root)
}

/// The user's home directory, from the environment.
fn home_dir() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .find_map(|key| std::env::var_os(key).filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

/// The options to serve with, or the exit status when the command line
/// asked for help or the version (printed here) or did not parse.
fn options_to_serve<I>(name: &str, version: &str, args: I) -> Result<Options, ExitCode>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    match parse(args) {
        Ok(Command::Serve(options)) => Ok(options),
        Ok(Command::Help) => {
            println!("{}", usage(name));
            Err(ExitCode::SUCCESS)
        }
        Ok(Command::Version) => {
            println!("{name} {version}");
            Err(ExitCode::SUCCESS)
        }
        Err(message) => {
            eprintln!("{name}: {message}\n\n{}", usage(name));
            Err(ExitCode::from(2))
        }
    }
}

/// Serve `factory`'s handler as `name` according to the command line.
///
/// This is the whole of `main`: parse, serve, and turn the outcome
/// into an exit status. A usage error prints the usage text and exits
/// with 2; a transport failure prints the reason and exits with 1.
pub fn run<H, F, I>(name: &str, version: &str, args: I, factory: F) -> ExitCode
where
    H: ServerHandler,
    F: Fn(PathBuf, ParseProfile) -> H + Send + Sync + 'static,
    I: IntoIterator,
    I::Item: Into<String>,
{
    let options = match options_to_serve(name, version, args) {
        Ok(options) => options,
        Err(code) => return code,
    };
    // The root is checked once, here, so a typo is a usage error with a
    // message instead of a server that refuses every file.
    let root = match chosen_root(&options) {
        Ok(root) => root,
        Err(message) => {
            eprintln!("{name}: {message}\n\n{}", usage(name));
            return ExitCode::from(2);
        }
    };
    let profile = options.profile;
    let make = move || factory(root.clone(), profile);
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("{name}: cannot start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve(&options, make)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{name}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Serve the handler over the chosen transport until the client goes
/// away (stdio) or the process is interrupted (HTTP).
///
/// # Errors
///
/// The listener could not bind, or the transport failed underneath the
/// session.
pub async fn serve<H, F>(options: &Options, factory: F) -> io::Result<()>
where
    H: ServerHandler,
    F: Fn() -> H + Send + Sync + 'static,
{
    match options.transport {
        Transport::Stdio => serve_stdio(factory()).await,
        Transport::StreamableHttp => {
            let listener = bind(options).await?;
            announce(&listener, STREAMABLE_HTTP_PATH)?;
            serve_streamable_http(listener, options, factory).await
        }
        Transport::Sse => {
            let listener = bind(options).await?;
            announce(&listener, SSE_PATH)?;
            serve_sse(listener, options, factory).await
        }
    }
}

async fn serve_stdio<H: ServerHandler>(handler: H) -> io::Result<()> {
    use rmcp::service::ServerInitializeError as Init;
    let running = match handler.serve(rmcp::transport::stdio()).await {
        Ok(running) => running,
        // The client hung up, or spoke before the handshake, and there
        // is nobody left to report to: a session that never began is
        // the normal end of a stdio server, not a failure of it.
        Err(Init::ConnectionClosed(_) | Init::ExpectedInitializeRequest(_)) => {
            return Ok(());
        }
        Err(e) => return Err(io::Error::other(format!("stdio session: {e}"))),
    };
    // Likewise once it is under way: the client closing the pipe is
    // how a stdio session ends.
    let _ = running
        .waiting()
        .await
        .map_err(|e| io::Error::other(format!("stdio session: {e}")))?;
    Ok(())
}

async fn bind(options: &Options) -> io::Result<TcpListener> {
    TcpListener::bind((options.host.as_str(), options.port))
        .await
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("cannot listen on {}:{}: {e}", options.host, options.port),
            )
        })
}

/// Say where the server is, on stderr so stdout stays free.
///
/// A test starts the server on port 0 and reads the port from here.
fn announce(listener: &TcpListener, path: &str) -> io::Result<()> {
    let addr = listener.local_addr()?;
    eprintln!("listening on http://{addr}{path}");
    Ok(())
}

/// Resolve when the process is asked to stop.
async fn interrupted() {
    if tokio::signal::ctrl_c().await.is_err() {
        // No signal handler could be installed: stay up until killed.
        std::future::pending::<()>().await;
    }
}

async fn serve_streamable_http<H, F>(
    listener: TcpListener,
    options: &Options,
    factory: F,
) -> io::Result<()>
where
    H: ServerHandler,
    F: Fn() -> H + Send + Sync + 'static,
{
    let ct = CancellationToken::new();
    let mut config =
        StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token());
    // The SDK refuses a `Host` header it does not expect, which is
    // what stops a page in a browser from reaching a local server
    // through DNS rebinding. Loopback names are allowed by default;
    // an operator who binds another interface has chosen to be
    // reachable by it, so that name is allowed too. Binding every
    // interface means there is no name to check against.
    // A browser page also carries its own origin; only a local one is
    // served. A client that is not a browser sends no `Origin`.
    config = match guard::allowed_hosts(&options.host) {
        Some(hosts) => config.with_allowed_hosts(hosts),
        None => config.disable_allowed_hosts(),
    }
    .with_allowed_origins(guard::allowed_origins(&options.host));
    let service = StreamableHttpService::new(
        move || Ok(factory()),
        CappedSessions::new(options.max_sessions).into(),
        config,
    );
    let router = Router::new().nest_service(STREAMABLE_HTTP_PATH, service);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            interrupted().await;
            ct.cancel();
        })
        .await
}

mod guard;
mod sessions;
mod sse;
use sessions::CappedSessions;
use sse::serve_sse;

#[cfg(test)]
mod tests;
