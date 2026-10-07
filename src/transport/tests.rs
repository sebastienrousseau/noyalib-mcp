// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Tests of the command line.

use super::*;

#[test]
fn no_arguments_means_stdio() {
    assert_eq!(
        parse(Vec::<String>::new()),
        Ok(Command::Serve(Options::default()))
    );
}

#[test]
fn the_http_transports_take_host_and_port() {
    let want = Options {
        transport: Transport::StreamableHttp,
        host: "0.0.0.0".to_owned(),
        port: 9000,
        root: None,
        profile: ParseProfile::Strict,
        max_sessions: DEFAULT_MAX_SESSIONS,
    };
    assert_eq!(
        parse([
            "--transport",
            "streamable-http",
            "--host",
            "0.0.0.0",
            "--port",
            "9000"
        ]),
        Ok(Command::Serve(want.clone()))
    );
    // `--flag=value` is the same as `--flag value`.
    assert_eq!(
        parse([
            "--transport=streamable-http",
            "--host=0.0.0.0",
            "--port=9000"
        ]),
        Ok(Command::Serve(want))
    );
    assert_eq!(
        parse(["--transport", "sse"]),
        Ok(Command::Serve(Options {
            transport: Transport::Sse,
            ..Options::default()
        }))
    );
}

#[test]
fn root_is_a_path_option() {
    assert_eq!(
        parse(["--root", "/srv/yaml"]),
        Ok(Command::Serve(Options {
            root: Some(PathBuf::from("/srv/yaml")),
            ..Options::default()
        }))
    );
    assert!(parse(["--root"]).is_err_and(|e| e.contains("needs a value")));
}

#[test]
fn profile_defaults_to_strict_and_takes_standard() {
    assert_eq!(Options::default().profile, ParseProfile::Strict);
    assert_eq!(
        parse(["--profile", "standard"]),
        Ok(Command::Serve(Options {
            profile: ParseProfile::Standard,
            ..Options::default()
        }))
    );
    assert!(parse(["--profile", "lax"]).is_err_and(|e| e.contains("lax")));
}

#[test]
fn max_sessions_takes_a_positive_count() {
    assert_eq!(
        parse(["--max-sessions", "3"]),
        Ok(Command::Serve(Options {
            max_sessions: 3,
            ..Options::default()
        }))
    );
    for bad in ["0", "-1", "many"] {
        assert!(parse(["--max-sessions", bad]).is_err_and(|e| e.contains(bad)));
    }
}

#[test]
fn a_missing_root_is_a_usage_error() {
    let err = resolve_root(Some(Path::new("/definitely/not/here"))).unwrap_err();
    assert!(err.contains("--root"), "{err}");
    assert!(resolve_root(None).is_ok());
}

#[test]
fn a_broad_working_directory_is_not_a_default_root() {
    let tmp = std::env::temp_dir();
    assert!(refuse_broad_default(Path::new("/"), None).is_err());
    assert!(refuse_broad_default(&tmp, Some(&tmp)).is_err());
    assert!(refuse_broad_default(&tmp, None).is_ok());
    assert!(refuse_broad_default(&tmp, Some(Path::new("/nonexistent"))).is_ok());
}

#[test]
fn help_and_version_win_over_everything_else() {
    assert_eq!(parse(["--help"]), Ok(Command::Help));
    assert_eq!(parse(["-h"]), Ok(Command::Help));
    assert_eq!(parse(["--version"]), Ok(Command::Version));
    assert_eq!(parse(["--transport", "sse", "-V"]), Ok(Command::Version));
}

#[test]
fn bad_arguments_are_named() {
    assert!(parse(["--transport", "carrier-pigeon"]).is_err_and(|e| e.contains("carrier-pigeon")));
    assert!(parse(["--port", "eighty"]).is_err_and(|e| e.contains("eighty")));
    assert!(parse(["--port", "70000"]).is_err_and(|e| e.contains("70000")));
    assert!(parse(["--port"]).is_err_and(|e| e.contains("needs a value")));
    assert!(parse(["--bogus"]).is_err_and(|e| e.contains("--bogus")));
}

#[test]
fn flags_split_on_the_first_equals_and_unknown_ones_stop_parsing() {
    // An unknown flag fails before anything after it is read, even a
    // flag that would otherwise win.
    assert!(parse(["--bogus", "--help"]).is_err_and(|e| e.contains("--bogus")));
    assert!(parse(["--bogus=1"]).is_err_and(|e| e.contains("`--bogus`")));
    // Only the first `=` separates the value.
    match parse(["--transport=sse", "--host=a=b", "--port=0"]) {
        Ok(Command::Serve(options)) => {
            assert_eq!(options.transport, Transport::Sse);
            assert_eq!(options.host, "a=b");
            assert_eq!(options.port, 0);
        }
        other => panic!("expected serve, got {other:?}"),
    }
    // A value-taking flag reads the next argument as its value.
    assert!(parse(["--host", "--help"]).is_ok_and(|c| matches!(
        c,
        Command::Serve(Options { ref host, .. }) if host == "--help"
    )));
}

#[test]
fn the_usage_text_names_every_flag_and_path() {
    let text = usage("any-mcp");
    for needle in [
        "any-mcp",
        "--transport",
        "--host",
        "--port",
        "--version",
        "--help",
        STREAMABLE_HTTP_PATH,
        SSE_PATH,
        MESSAGE_PATH,
    ] {
        assert!(text.contains(needle), "usage lacks {needle}:\n{text}");
    }
}
