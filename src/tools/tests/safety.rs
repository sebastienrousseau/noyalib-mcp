// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! What the tools refuse: paths outside the root, races on the way in,
//! oversized or over-deep input, and documents the profile rejects.

use super::*;

// ── file safety ──────────────────────────────────────────────────

/// A fresh directory under the system temp dir.
fn scratch_dir(label: &str) -> PathBuf {
    let p = temp_path(label).with_extension("d");
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

#[cfg(unix)]
#[test]
fn set_keeps_the_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch_dir("mode");
    let server = YamlServer::with_root(root.clone());
    for mode in [0o600, 0o755, 0o640] {
        let name = format!("m{mode:o}.yml");
        let p = root.join(&name);
        fs::write(&p, "a: 1\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        let r = call_on(
            &server,
            "noyalib_set",
            json!({"file": name, "path": "a", "value": "2"}),
        );
        assert!(!is_error(&r), "{}", text_of(&r));
        assert_eq!(fs::read_to_string(&p).unwrap(), "a: 2\n");
        let got = fs::metadata(&p).unwrap().permissions().mode() & 0o7777;
        assert_eq!(got, mode, "mode of {name}: {got:o}");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn outside_the_root_is_one_answer_whatever_is_there() {
    // Whether a path outside the root exists, is missing or is
    // unreadable must not show, and neither may the root itself.
    let root = scratch_dir("oracle");
    let outside = write_temp("oracle-outside", "a: 1\n");
    let server = YamlServer::with_root(root.clone());
    let missing = outside.with_extension("missing");
    let mut answers = Vec::new();
    for file in [
        outside.to_str().unwrap().to_owned(),
        missing.to_str().unwrap().to_owned(),
        format!("../{}", outside.file_name().unwrap().to_str().unwrap()),
        format!("../{}", missing.file_name().unwrap().to_str().unwrap()),
        "/etc/does-not-exist".to_owned(),
    ] {
        let r = call_on(&server, "noyalib_get", json!({"file": file, "path": "a"}));
        assert!(is_error(&r));
        let text = text_of(&r).replace(&file, "<file>");
        assert!(!text.contains(root.to_str().unwrap()), "{text}");
        assert!(!text.contains(server.root().to_str().unwrap()), "{text}");
        answers.push(text);
    }
    assert!(answers.windows(2).all(|w| w[0] == w[1]), "{answers:#?}");
    assert!(
        answers[0].contains("outside the server root"),
        "{}",
        answers[0]
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside);
}

#[cfg(unix)]
#[test]
fn a_racing_symlink_swap_never_escapes_the_root() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    let root = scratch_dir("race-root");
    let outside = scratch_dir("race-outside");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/secret.yml"), "secret: inside\n").unwrap();
    fs::write(outside.join("secret.yml"), "secret: OUTSIDE\n").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (root, outside, stop) = (root.clone(), outside.clone(), Arc::clone(&stop));
        std::thread::spawn(move || {
            let (sub, real) = (root.join("sub"), root.join("sub.real"));
            while !stop.load(Ordering::Relaxed) {
                let _ = fs::rename(&sub, &real);
                let _ = std::os::unix::fs::symlink(&outside, &sub);
                let _ = fs::remove_file(&sub);
                let _ = fs::rename(&real, &sub);
            }
        })
    };
    let server = YamlServer::with_root(root.clone());
    let file = json!("sub/secret.yml");
    for i in 0..6000 {
        let r = call_on(
            &server,
            "noyalib_get",
            json!({"file": file, "path": "secret"}),
        );
        assert_ne!(text_of(&r), "OUTSIDE", "read escaped the root on call {i}");
        if i % 4 == 0 {
            let v = format!("w{i}");
            let _ = call_on(
                &server,
                "noyalib_set",
                json!({"file": file, "path": "secret", "value": v}),
            );
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
    assert_eq!(
        fs::read_to_string(outside.join("secret.yml")).unwrap(),
        "secret: OUTSIDE\n",
        "a write escaped the root"
    );
    let stray: Vec<_> = fs::read_dir(&outside)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(stray.len(), 1, "temp files landed outside: {stray:?}");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

// ── request limits ───────────────────────────────────────────────

#[test]
fn nesting_depth_never_under_counts() {
    let cases = [
        ("1", 1),
        ("[1, [2, [3]]]", 4),
        ("{a: {b: [c]}}", 4),
        ("- - - x", 4),
        ("a:\n  b:\n    c: 1\n", 4),
        ("a:\n- b\n- c\n", 2),
        ("[\n [\n  [\n  ]\n ]\n]\n", 6),
        ("? a\n: b\n", 2),
    ];
    for (text, at_least) in cases {
        let got = nesting_depth(text);
        assert!(got >= at_least, "{text:?}: {got} < {at_least}");
    }
    assert_eq!(nesting_depth(&"[".repeat(100_000)), 100_001);
    assert_eq!(nesting_depth(&"- ".repeat(100_000)), 100_001);
    assert!(nesting_depth("plain scalar text") < 3);
}

#[test]
fn fragments_are_checked_against_the_profile() {
    let strict = ParseProfile::Strict.config();
    let deep = "[".repeat(65) + &"]".repeat(65);
    let err = check_fragment(&deep, &strict).unwrap_err();
    assert!(err.contains("over the limit of 64"), "{err}");
    assert!(check_fragment(&deep, &ParseProfile::Standard.config()).is_ok());
    let long = "x".repeat(MAX_FRAGMENT_BYTES + 1);
    let err = check_fragment(&long, &strict).unwrap_err();
    assert!(err.contains("fragment limit"), "{err}");
    assert!(check_fragment("[1, 2, {a: b}]", &strict).is_ok());
}

#[test]
fn edit_refuses_a_deep_fragment_without_echoing_it() {
    let deep = "[".repeat(100_000) + &"]".repeat(100_000);
    let err = edit_with_profile("a: 1\n", "a", &deep, ParseProfile::Strict).unwrap_err();
    assert!(err.contains("nests"), "{err}");
    // A 40 KB fragment the document refuses is named, not repeated.
    let bad = format!("\"{}", "x".repeat(40_000));
    let err = edit("a: 1\n", "a", &bad).unwrap_err();
    assert!(err.len() < 600, "{} bytes: {err}", err.len());
    assert!(err.contains("(40001 bytes)"), "{err}");
}

#[test]
fn clip_keeps_short_text_and_cuts_on_a_char_boundary() {
    assert_eq!(clip("short"), "short");
    let long = "é".repeat(100);
    let clipped = clip(&long);
    assert!(clipped.ends_with("... (200 bytes)"), "{clipped}");
    assert!(clipped.len() < 140);
}

#[test]
fn text_over_the_document_limit_is_refused_before_parsing() {
    let long = format!("a: {}\n", "x".repeat(1 << 20));
    let err = parse(&long).unwrap_err();
    assert!(err.contains("document limit"), "{err}");
    let err = edit_with_profile(&long, "a", "1", ParseProfile::Strict).unwrap_err();
    assert!(err.contains("document limit"), "{err}");
    let verdict = validate(&long, None).unwrap();
    assert!(!verdict.valid);
    assert!(verdict.error.unwrap().contains("document limit"));
    assert!(parse_with_profile(&long, ParseProfile::Standard).is_ok());
}

#[test]
fn a_schema_over_the_size_limit_is_refused_uncompiled() {
    let big = format!(
        "{{\"type\":\"object\",\"description\":\"{}\"}}",
        "x".repeat(MAX_SCHEMA_BYTES)
    );
    let err = validate("a: 1\n", Some(&big)).unwrap_err();
    assert!(err.contains("schema limit"), "{err}");
}

#[test]
fn violations_are_capped_and_clipped() {
    // Every item fails twice, with a long message: the verdict keeps
    // the first MAX_VIOLATIONS and says the list was cut short.
    let yaml: String = (0..500)
        .map(|_| format!("- {}\n", "y".repeat(300)))
        .collect();
    let schema = r#"{"type":"array","items":{"type":"integer"}}"#;
    let out = validate_with_profile(&yaml, Some(schema), ParseProfile::Standard).unwrap();
    assert!(!out.valid);
    assert_eq!(
        out.violations.len(),
        MAX_VIOLATIONS + 1,
        "{:?}",
        out.violations.last()
    );
    let last = out.violations.last().unwrap();
    assert_eq!(last.keyword, "truncated");
    assert!(last.message.contains("more than"), "{}", last.message);
    assert!(out.violations.iter().all(|v| v.message.len() < 400));
}

#[test]
fn exactly_the_cap_is_listed_without_a_truncation_entry() {
    let yaml: String = (0..MAX_VIOLATIONS).map(|_| "- y\n").collect();
    let schema = r#"{"type":"array","items":{"type":"integer"}}"#;
    let out = validate_with_profile(&yaml, Some(schema), ParseProfile::Standard).unwrap();
    assert_eq!(out.violations.len(), MAX_VIOLATIONS);
    assert!(out.violations.iter().all(|v| v.keyword != "truncated"));
}

#[test]
fn a_call_over_the_time_limit_is_answered_with_an_error() {
    let long: String = (0..200_000)
        .map(|i| format!("k{i}: [1, 2, {{a: b}}]\n"))
        .collect();
    let server = YamlServer::with_root(std::env::temp_dir())
        .with_profile(ParseProfile::Standard)
        .with_call_timeout(std::time::Duration::from_millis(1));
    let r = call_on(&server, "noyalib_parse", json!({"yaml": long}));
    assert!(is_error(&r));
    assert!(text_of(&r).contains("time limit"), "{}", text_of(&r));
    let r = call_on(
        &server.with_call_timeout(DEFAULT_CALL_TIMEOUT),
        "noyalib_parse",
        json!({"yaml": "a: 1"}),
    );
    assert!(!is_error(&r), "{}", text_of(&r));
}

#[test]
fn the_cst_tools_follow_the_profile() {
    // Strict refuses duplicate keys; the CST tools now agree with
    // noyalib_parse instead of taking the last value.
    let root = scratch_dir("cst-profile");
    fs::write(root.join("dup.yml"), "a: 1\na: 2\n").unwrap();
    fs::write(root.join("dup2.yml"), "x: 0\n---\na: 1\na: 2\n").unwrap();
    let strict = YamlServer::with_root(root.clone());
    let dup = "a: 1\na: 2\n";
    for (tool, a) in [
        ("noyalib_get", json!({"file": "dup.yml", "path": "a"})),
        (
            "noyalib_set",
            json!({"file": "dup.yml", "path": "a", "value": "3"}),
        ),
        (
            "noyalib_set_multidoc",
            json!({"file": "dup2.yml", "doc_index": 1, "path": "a", "value": "3"}),
        ),
        (
            "noyalib_edit",
            json!({"yaml": dup, "path": "a", "value": "3"}),
        ),
    ] {
        let r = call_on(&strict, tool, a.clone());
        assert!(is_error(&r), "{tool}: {}", text_of(&r));
        assert!(
            text_of(&r).to_lowercase().contains("duplicate"),
            "{tool}: {}",
            text_of(&r)
        );
        let standard = strict.clone().with_profile(ParseProfile::Standard);
        let r = call_on(&standard, tool, a);
        assert!(!is_error(&r), "{tool} under standard: {}", text_of(&r));
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_directory_is_not_read_as_a_file() {
    let dir = scratch_dir("isdir");
    let err = get(dir.to_str().unwrap(), "a").unwrap_err();
    assert!(err.starts_with("read "), "{err}");
    assert!(err.contains("not a regular file"), "{err}");
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn a_write_the_directory_refuses_leaves_the_file_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_dir("rodir");
    let one = dir.join("one.yml");
    let stream = dir.join("stream.yml");
    fs::write(&one, "a: 1\n").unwrap();
    fs::write(&stream, "a: 1\n---\nb: 2\n").unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    // A privileged user writes regardless of the mode; nothing to see.
    let privileged = fs::write(dir.join("probe"), "").is_ok();
    let set_err = set(one.to_str().unwrap(), "a", "9");
    let multidoc_err = set_multidoc(stream.to_str().unwrap(), 1, "b", "9");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    if !privileged {
        let err = set_err.unwrap_err();
        assert!(err.starts_with("write "), "{err}");
        let err = multidoc_err.unwrap_err();
        assert!(err.starts_with("write "), "{err}");
        assert_eq!(fs::read_to_string(&one).unwrap(), "a: 1\n");
        assert_eq!(fs::read_to_string(&stream).unwrap(), "a: 1\n---\nb: 2\n");
    }
    let _ = fs::remove_dir_all(dir);
}
