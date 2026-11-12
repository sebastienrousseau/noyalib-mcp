// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Actual SDK handlers share capacity after timed-out blocking work.
//!
//! This integration binary owns its runtime and process-wide budget, so
//! parallel unit tests cannot consume its permits or blocking threads.

#![allow(missing_docs)]

use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::service::{RoleClient, RunningService};
use std::time::Duration;

struct ReleaseOnDrop(Option<std::sync::mpsc::Sender<()>>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

async fn session(timeout: Duration) -> RunningService<RoleClient, ()> {
    let (client_io, server_io) = tokio::io::duplex(1 << 16);
    drop(tokio::spawn(async move {
        if let Ok(server) = noyalib_mcp::YamlServer::new()
            .with_call_timeout(timeout)
            .serve(server_io)
            .await
        {
            let _ = server.waiting().await;
        }
    }));
    ().serve(client_io).await.expect("SDK handshake")
}

fn request() -> CallToolRequestParams {
    CallToolRequestParams::new("noyalib_parse").with_arguments(
        serde_json::json!({ "yaml": "key: value\n" })
            .as_object()
            .unwrap()
            .clone(),
    )
}

#[test]
fn timed_out_jobs_share_capacity_across_sdk_handlers_and_release_it() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let (release, held) = std::sync::mpsc::channel();
    let release = ReleaseOnDrop(Some(release));
    runtime.block_on(async move {
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            held.recv().unwrap();
        });
        tokio::time::timeout(Duration::from_secs(5), ready)
            .await
            .expect("blocking pool occupied")
            .unwrap();

        let first = session(Duration::from_millis(20)).await;
        let second = session(Duration::from_millis(20)).await;
        for index in 0..64 {
            let client = if index % 2 == 0 { &first } else { &second };
            let result = tokio::time::timeout(Duration::from_secs(5), client.call_tool(request()))
                .await
                .expect("tool response deadline")
                .expect("tool result");
            assert_eq!(result.is_error, Some(true), "call {index}: {result:?}");
            assert!(
                format!("{result:?}").contains("time limit"),
                "call {index} must time out while its blocking job remains queued: {result:?}"
            );
        }
        let overflow = second.call_tool(request()).await.unwrap();
        assert_eq!(overflow.is_error, Some(true));
        assert!(
            format!("{overflow:?}").contains("server is busy"),
            "capacity must remain shared across handlers after all timeouts: {overflow:?}"
        );

        drop(release);
        blocker.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), tokio::task::spawn_blocking(|| ()))
            .await
            .expect("queued jobs drain")
            .unwrap();
        let recovered = session(Duration::from_secs(5)).await;
        let result = recovered.call_tool(request()).await.unwrap();
        assert_ne!(result.is_error, Some(true), "capacity returns: {result:?}");
        assert_eq!(
            result.structured_content,
            Some(serde_json::json!({ "documents": [{ "key": "value" }] }))
        );
        first.cancel().await.unwrap();
        second.cancel().await.unwrap();
        recovered.cancel().await.unwrap();
    });
}
