// SPDX-FileCopyrightText: 2026 Noyalib
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exercise the actual capped SDK manager against a bounded lifecycle model.
//! Transport handles stay alive until close; concurrent admissions must share
//! the same ceiling. No network listener or filesystem input is involved.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rmcp::transport::streamable_http_server::session::{RestoreOutcome, SessionId, SessionManager};
use std::sync::{Arc, OnceLock};

#[path = "../../src/transport/sessions.rs"]
mod sessions;

use sessions::{CappedError, CappedSessions};

type LiveSession = (SessionId, <CappedSessions as SessionManager>::Transport);

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("lifecycle runtime")
    })
}

fn full(error: CappedError, cap: usize) {
    match error {
        CappedError::Full(actual) => assert_eq!(actual, cap),
        CappedError::Inner(error) => panic!("unexpected manager failure: {error}"),
    }
}

async fn close_all(manager: &CappedSessions, live: &mut Vec<LiveSession>) {
    while let Some((id, transport)) = live.pop() {
        manager.close_session(&id).await.unwrap();
        assert!(!manager.has_session(&id).await.unwrap());
        drop(transport);
    }
    tokio::task::yield_now().await;
}

async fn exercise(data: &[u8]) {
    let cap = usize::from(data.first().copied().unwrap_or(0) % 4) + 1;
    let manager = Arc::new(CappedSessions::new(cap));
    let mut live = Vec::new();
    for (step, byte) in data.iter().skip(1).take(32).enumerate() {
        match byte % 6 {
            0 => match manager.create_session().await {
                Ok(session) => {
                    assert!(live.len() < cap);
                    live.push(session);
                }
                Err(error) => {
                    full(error, cap);
                    assert_eq!(live.len(), cap);
                }
            },
            1 => {
                let id: SessionId = if byte & 0x80 != 0 && !live.is_empty() {
                    live[usize::from(*byte) % live.len()].0.clone()
                } else {
                    format!("restore-{step}").into()
                };
                let present = live.iter().any(|(known, _)| known == &id);
                match manager.restore_session(id.clone()).await {
                    Ok(RestoreOutcome::AlreadyPresent) => assert!(present),
                    Ok(RestoreOutcome::Restored(transport)) => {
                        assert!(!present && live.len() < cap);
                        live.push((id, transport));
                    }
                    Ok(_) => panic!("unexpected restore outcome"),
                    Err(error) => {
                        full(error, cap);
                        assert!(!present);
                        assert_eq!(live.len(), cap);
                    }
                }
            }
            2 if !live.is_empty() => {
                let (id, transport) = live.swap_remove(usize::from(*byte) % live.len());
                manager.close_session(&id).await.unwrap();
                assert!(!manager.has_session(&id).await.unwrap());
                drop(transport);
            }
            4 => {
                let barrier = Arc::new(tokio::sync::Barrier::new(3));
                let mut tasks = Vec::new();
                for _ in 0..2 {
                    let manager = Arc::clone(&manager);
                    let barrier = Arc::clone(&barrier);
                    tasks.push(tokio::spawn(async move {
                        barrier.wait().await;
                        manager.create_session().await
                    }));
                }
                barrier.wait().await;
                let mut refused = false;
                for task in tasks {
                    match task.await.unwrap() {
                        Ok(session) => live.push(session),
                        Err(error) => {
                            full(error, cap);
                            refused = true;
                        }
                    }
                }
                assert!(live.len() <= cap, "concurrent admission exceeded the cap");
                if refused {
                    assert_eq!(live.len(), cap);
                }
            }
            5 => close_all(&manager, &mut live).await,
            _ => {
                let missing: SessionId = "missing".into();
                assert!(!manager.has_session(&missing).await.unwrap());
                manager.close_session(&missing).await.unwrap();
            }
        }
        assert!(live.len() <= cap);
        for (id, _) in &live {
            assert!(manager.has_session(id).await.unwrap());
        }
    }
    close_all(&manager, &mut live).await;
    let (id, transport) = manager
        .create_session()
        .await
        .expect("closed capacity is reusable");
    manager.close_session(&id).await.unwrap();
    drop(transport);
    tokio::task::yield_now().await;
}

fuzz_target!(|data: &[u8]| runtime().block_on(exercise(data)));
