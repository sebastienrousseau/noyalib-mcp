// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Tests of the walk, the read limits and the replacement.

use super::*;
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!(
        "noyalib-mcp-fsio-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn read(root: &RootDir, file: &str) -> Result<String, FileError> {
    let at = root.locate(Path::new(file))?;
    Ok(at.read(1 << 20)?.0)
}

#[test]
fn relative_paths_and_dot_dot_inside_the_root_resolve() {
    let dir = scratch("rel");
    fs::create_dir_all(dir.join("a/b")).unwrap();
    fs::write(dir.join("a/x.yml"), "x: 1\n").unwrap();
    let root = RootDir::open(dir.clone());
    assert_eq!(read(&root, "a/b/../x.yml").unwrap(), "x: 1\n");
    assert_eq!(read(&root, "./a/x.yml").unwrap(), "x: 1\n");
    let absolute = dir.join("a/x.yml");
    assert_eq!(read(&root, absolute.to_str().unwrap()).unwrap(), "x: 1\n");
    assert!(matches!(read(&root, "../x.yml"), Err(FileError::Outside)));
    assert!(matches!(
        read(&root, "a/../../x.yml"),
        Err(FileError::Outside)
    ));
    assert!(matches!(read(&root, "/etc/hosts"), Err(FileError::Outside)));
    assert!(matches!(
        read(&root, "a/missing.yml"),
        Err(FileError::Io(_))
    ));
    assert!(matches!(read(&root, "a"), Err(FileError::Io(_))));
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn symlinks_are_followed_inside_the_root_only() {
    use std::os::unix::fs::symlink;
    let dir = scratch("links");
    let outside = scratch("links-outside");
    fs::create_dir_all(dir.join("conf")).unwrap();
    fs::write(dir.join("conf/real.yml"), "r: 1\n").unwrap();
    fs::write(outside.join("o.yml"), "o: 1\n").unwrap();
    symlink("conf/real.yml", dir.join("rel.yml")).unwrap();
    symlink("conf", dir.join("confdir")).unwrap();
    let canonical = dir.canonicalize().unwrap();
    symlink(canonical.join("conf/real.yml"), dir.join("abs.yml")).unwrap();
    symlink(outside.join("o.yml"), dir.join("out.yml")).unwrap();
    symlink("../", dir.join("conf/up")).unwrap();
    symlink("loop", dir.join("loop")).unwrap();
    let root = RootDir::open(dir.clone());
    for inside in ["rel.yml", "abs.yml", "confdir/real.yml", "conf/up/rel.yml"] {
        assert_eq!(read(&root, inside).unwrap(), "r: 1\n", "{inside}");
    }
    for escape in ["out.yml", "conf/up/../x.yml", "conf/up/conf/up/../x.yml"] {
        assert!(
            matches!(read(&root, escape), Err(FileError::Outside)),
            "{escape}"
        );
    }
    assert!(matches!(read(&root, "loop"), Err(FileError::Io(_))));
    // Writing through an in-root symlink replaces its target, not
    // the link.
    let at = root.locate(Path::new("rel.yml")).unwrap();
    let (_, kept) = at.read(1 << 20).unwrap();
    at.replace(b"r: 2\n", kept).unwrap();
    assert!(
        fs::symlink_metadata(dir.join("rel.yml"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(dir.join("conf/real.yml")).unwrap(),
        "r: 2\n"
    );
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn a_planted_temp_name_is_never_opened() {
    // Someone who can create files beside the target plants a
    // symlink at the name the replacement will try first. The
    // exclusive create refuses it and the next name is used.
    use std::os::unix::fs::symlink;
    let dir = scratch("planted");
    let outside = scratch("planted-outside");
    fs::write(dir.join("t.yml"), "a: 1\n").unwrap();
    fs::write(outside.join("victim"), "untouched\n").unwrap();
    symlink(outside.join("victim"), dir.join(".planted")).unwrap();
    let root = RootDir::open(dir.clone());
    let at = root.locate(Path::new("t.yml")).unwrap();
    let (_, kept) = at.read(1 << 20).unwrap();
    let mut first = true;
    let mut names = || {
        if std::mem::take(&mut first) {
            OsString::from(".planted")
        } else {
            OsString::from(".fresh")
        }
    };
    at.replace_named(b"a: 2\n", kept, &mut names).unwrap();
    assert_eq!(fs::read_to_string(dir.join("t.yml")).unwrap(), "a: 2\n");
    assert_eq!(
        fs::read_to_string(outside.join("victim")).unwrap(),
        "untouched\n"
    );
    assert!(
        fs::symlink_metadata(dir.join(".planted"))
            .unwrap()
            .is_symlink()
    );
    assert!(!dir.join(".fresh").exists(), "the temp was renamed away");
    // Every name taken: the replacement gives up, file unchanged.
    let mut always = || OsString::from(".planted");
    let err = at.replace_named(b"a: 3\n", kept, &mut always).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(dir.join("t.yml")).unwrap(), "a: 2\n");
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn temp_names_are_hidden_random_and_beside_the_target() {
    let a = temp_name(&OsString::from("c.yml"));
    let b = temp_name(&OsString::from("c.yml"));
    assert_ne!(a, b);
    let a = a.to_str().unwrap();
    assert!(a.starts_with(".c.yml.") && a.ends_with(".tmp"), "{a}");
    assert_eq!(a.len(), ".c.yml.".len() + 32 + ".tmp".len());
}

#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_name_is_opened_as_itself() {
    // The walk never turns a path into a string, so a symlink to a
    // non-UTF-8 name cannot be read as a lossy look-alike.
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    let dir = scratch("lossy");
    let raw = std::ffi::OsStr::from_bytes(b"\xff.yml");
    fs::write(dir.join(raw), "a: raw\n").unwrap();
    fs::write(dir.join("\u{FFFD}.yml"), "a: lookalike\n").unwrap();
    symlink(raw, dir.join("link.yml")).unwrap();
    let root = RootDir::open(dir.clone());
    assert_eq!(read(&root, "link.yml").unwrap(), "a: raw\n");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_root_that_cannot_be_opened_refuses_every_lookup() {
    let dir = scratch("gone");
    let missing = dir.join("not-here");
    let root = RootDir::open(missing.clone());
    assert_eq!(root.path(), missing);
    let shown = format!("{root:?}");
    assert!(
        shown.starts_with("RootDir") && shown.contains("not-here"),
        "{shown}"
    );
    let message = match root.locate(Path::new("x.yml")) {
        Err(FileError::Io(e)) => e.to_string(),
        _ => panic!("a lookup under a missing root must fail"),
    };
    #[cfg(unix)]
    assert!(message.contains("cannot be opened"), "{message}");
    assert!(!message.is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn only_a_regular_file_within_the_limit_is_read() {
    let dir = scratch("limit");
    fs::write(dir.join("t.yml"), "a: 12345\n").unwrap();
    let root = RootDir::open(dir.clone());
    let at = root.locate(Path::new("t.yml")).unwrap();
    let err = at.read(4).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("9 bytes, over the 4-byte"),
        "{err}"
    );
    assert_eq!(at.read(9).unwrap().0, "a: 12345\n");
    // A directory opens for reading and is then refused.
    let err = Located::unconfined(&dir)
        .unwrap()
        .read(1 << 20)
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("not a regular file"), "{err}");
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn a_path_that_names_no_file_is_refused() {
    let err = Located::unconfined(Path::new("/")).err().unwrap();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    let dir = scratch("nofile");
    fs::create_dir_all(dir.join("a")).unwrap();
    let root = RootDir::open(dir.clone());
    for walk_ends_on_a_dir in [".", "a/..", "a/."] {
        assert!(
            matches!(read(&root, walk_ends_on_a_dir), Err(FileError::Io(_))),
            "{walk_ends_on_a_dir}"
        );
    }
    assert!(matches!(
        unix::components(Path::new("/a")),
        Err(FileError::Outside)
    ));
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn a_failed_rename_removes_the_temp_and_keeps_the_target() {
    // The target is a non-empty directory: the temp is created and
    // written, the rename over the directory fails, and the temp is
    // removed rather than left beside it.
    let dir = scratch("rename");
    fs::create_dir_all(dir.join("d/inner")).unwrap();
    let at = Located::unconfined(&dir.join("d")).unwrap();
    let keep = kept(&fs::metadata(dir.join("d")).unwrap());
    let mut names = || OsString::from(".d.tmp");
    assert!(at.replace_named(b"x: 1\n", keep, &mut names).is_err());
    assert!(!dir.join(".d.tmp").exists(), "the temp was removed");
    assert!(dir.join("d/inner").is_dir());
    let _ = fs::remove_dir_all(dir);
}
