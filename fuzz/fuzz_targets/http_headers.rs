// SPDX-FileCopyrightText: 2026 Noyalib
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host, Origin and content-type parsing must be total and reject foreign origins.

#![no_main]

#[path = "../../src/transport/guard.rs"]
#[allow(dead_code)]
mod guard;

use axum::http::HeaderMap;
use axum::http::header::{CONTENT_TYPE, HOST, ORIGIN};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut parts = text.splitn(3, '\0');
    let mut headers = HeaderMap::new();
    for (name, value) in [HOST, ORIGIN, CONTENT_TYPE].into_iter().zip(&mut parts) {
        if let Ok(value) = value.parse() {
            let _ = headers.insert(name, value);
        }
    }
    let local = guard::Guard::for_bind_host("127.0.0.1");
    let _ = local.check(&headers);
    let _ = guard::require_json(&headers);

    let _ = headers.insert(HOST, "attacker.invalid".parse().unwrap());
    assert!(local.check(&headers).is_err());
    let _ = headers.insert(HOST, "localhost".parse().unwrap());
    let _ = headers.insert(ORIGIN, "https://attacker.invalid".parse().unwrap());
    assert!(local.check(&headers).is_err());
    let _ = headers.insert(CONTENT_TYPE, "text/plain".parse().unwrap());
    assert!(guard::require_json(&headers).is_err());
});
