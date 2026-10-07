// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! A session manager that holds at most so many sessions at once.
//!
//! Every streamable HTTP `initialize` creates a session that lives
//! until the client closes it or it idles out (minutes). Without a
//! bound, a client could open sessions faster than they expire. This
//! wraps the SDK's in-memory manager and refuses to create one past
//! the limit; everything else is passed through.

use std::fmt;
use std::sync::Arc;

use futures::Stream;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::streamable_http_server::session::local::{
    LocalSessionManager, LocalSessionManagerError,
};
use rmcp::transport::streamable_http_server::session::{
    EventStore, RestoreOutcome, ServerSseMessage, SessionId, SessionManager,
};

/// [`LocalSessionManager`] with a ceiling on live sessions.
#[derive(Debug, Default)]
pub(super) struct CappedSessions {
    inner: LocalSessionManager,
    max: usize,
}

impl CappedSessions {
    pub(super) fn new(max: usize) -> Self {
        Self {
            inner: LocalSessionManager::default(),
            max,
        }
    }
}

/// Why a session operation failed.
#[derive(Debug)]
pub(super) enum CappedError {
    /// Creating one more session would pass the limit.
    Full(usize),
    /// The wrapped manager failed.
    Inner(LocalSessionManagerError),
}

impl fmt::Display for CappedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full(max) => write!(
                f,
                "the server holds its session limit of {max}; close a session or retry later"
            ),
            Self::Inner(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CappedError {}

type Transport = <LocalSessionManager as SessionManager>::Transport;
type Outcome<T> = Result<T, CappedError>;

impl SessionManager for CappedSessions {
    type Error = CappedError;
    type Transport = Transport;

    async fn create_session(&self) -> Outcome<(SessionId, Transport)> {
        if self.inner.sessions.read().await.len() >= self.max {
            return Err(CappedError::Full(self.max));
        }
        self.inner
            .create_session()
            .await
            .map_err(CappedError::Inner)
    }

    async fn initialize_session(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Outcome<ServerJsonRpcMessage> {
        let reply = self.inner.initialize_session(id, message).await;
        reply.map_err(CappedError::Inner)
    }

    async fn has_session(&self, id: &SessionId) -> Outcome<bool> {
        self.inner.has_session(id).await.map_err(CappedError::Inner)
    }

    async fn close_session(&self, id: &SessionId) -> Outcome<()> {
        self.inner
            .close_session(id)
            .await
            .map_err(CappedError::Inner)
    }

    async fn create_stream(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Outcome<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static> {
        let stream = self.inner.create_stream(id, message).await;
        stream.map_err(CappedError::Inner)
    }

    async fn accept_message(&self, id: &SessionId, message: ClientJsonRpcMessage) -> Outcome<()> {
        let accepted = self.inner.accept_message(id, message).await;
        accepted.map_err(CappedError::Inner)
    }

    async fn create_standalone_stream(
        &self,
        id: &SessionId,
    ) -> Outcome<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static> {
        let stream = self.inner.create_standalone_stream(id).await;
        stream.map_err(CappedError::Inner)
    }

    async fn resume(
        &self,
        id: &SessionId,
        last_event_id: String,
    ) -> Outcome<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static> {
        let stream = self.inner.resume(id, last_event_id).await;
        stream.map_err(CappedError::Inner)
    }

    async fn restore_session(&self, id: SessionId) -> Outcome<RestoreOutcome<Transport>> {
        let restored = self.inner.restore_session(id).await;
        restored.map_err(CappedError::Inner)
    }

    fn event_store(&self) -> Option<Arc<dyn EventStore>> {
        self.inner.event_store()
    }
}
