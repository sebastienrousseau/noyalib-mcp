// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Finding, reading and replacing the files the file tools work on.
//!
//! On unix the server opens its root once, as a directory handle, and
//! walks every `file` argument from that handle one component at a
//! time with `O_NOFOLLOW`. A symlink met on the way is read and its
//! target walked the same way, so a symlink that stays inside the root
//! works and one that leaves it is refused. Nothing is ever reopened
//! by name from the top, so a directory swapped for a symlink between
//! a check and an open cannot redirect the read or the write: there is
//! no check separate from the open. The file is then read, and its
//! replacement written and renamed into place, relative to the
//! directory handle the walk ended in.
//!
//! Elsewhere the path is canonicalised and compared with the root,
//! which is what the platform offers; the race above remains there.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read as _, Write as _};
#[cfg(not(unix))]
use std::path::Component;
use std::path::{Path, PathBuf};

/// Why a file could not be located.
#[derive(Debug)]
pub(crate) enum FileError {
    /// The path, or a symlink on it, leads outside the root. Nothing
    /// outside the root was looked at to decide this.
    Outside,
    /// The path is inside the root and the system refused it.
    Io(io::Error),
}

impl From<io::Error> for FileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// How many temp names a replacement tries before giving up.
const TEMP_ATTEMPTS: usize = 8;

/// The server root: its canonical path, the path it was given as, and
/// (on unix) the directory handle every walk starts from.
#[derive(Clone)]
pub(crate) struct RootDir {
    canonical: PathBuf,
    given: PathBuf,
    #[cfg(unix)]
    handle: Option<std::sync::Arc<std::os::fd::OwnedFd>>,
}

impl fmt::Debug for RootDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootDir")
            .field("canonical", &self.canonical)
            .finish_non_exhaustive()
    }
}

impl RootDir {
    /// Open `root`. A root that cannot be opened makes every later
    /// lookup fail with the reason, rather than failing here.
    pub(crate) fn open(root: PathBuf) -> Self {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
        Self {
            #[cfg(unix)]
            handle: std::fs::File::open(&canonical)
                .ok()
                .map(|f| std::sync::Arc::new(std::os::fd::OwnedFd::from(f))),
            canonical,
            given: root,
        }
    }

    /// The canonical root.
    pub(crate) fn path(&self) -> &Path {
        &self.canonical
    }

    /// `file` relative to the root: a relative path as it is, an
    /// absolute one with the root stripped. Purely lexical, so an
    /// absolute path elsewhere is refused without being looked at.
    fn relative<'a>(&self, file: &'a Path) -> Result<&'a Path, FileError> {
        if !file.is_absolute() {
            return Ok(file);
        }
        file.strip_prefix(&self.canonical)
            .or_else(|_| file.strip_prefix(&self.given))
            .map_err(|_| FileError::Outside)
    }

    /// Find `file` under the root.
    pub(crate) fn locate(&self, file: &Path) -> Result<Located, FileError> {
        let relative = self.relative(file)?;
        self.walk(relative)
    }

    #[cfg(unix)]
    fn walk(&self, relative: &Path) -> Result<Located, FileError> {
        let handle = self.handle.as_ref().ok_or_else(|| {
            FileError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "the server root cannot be opened",
            ))
        })?;
        unix::walk(handle, &self.canonical, relative)
    }

    #[cfg(not(unix))]
    fn walk(&self, relative: &Path) -> Result<Located, FileError> {
        let joined = self.canonical.join(relative);
        if !lexically_inside(relative) {
            return Err(FileError::Outside);
        }
        let resolved = joined.canonicalize()?;
        if !resolved.starts_with(&self.canonical) {
            return Err(FileError::Outside);
        }
        Ok(Located::from_path(&resolved)?)
    }
}

/// Whether `relative` stays inside its base when read without
/// following anything: no root, prefix or `..` that climbs above it.
#[cfg(not(unix))]
fn lexically_inside(relative: &Path) -> bool {
    let mut depth = 0usize;
    for c in relative.components() {
        match c {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// A file found: the directory it is in, and its name there.
pub(crate) struct Located {
    #[cfg(unix)]
    dir: std::os::fd::OwnedFd,
    #[cfg(not(unix))]
    dir: PathBuf,
    name: OsString,
}

/// What a replacement keeps from the file it replaces.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Kept {
    /// Permission bits (unix), or the read-only flag elsewhere.
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    owner: (u32, u32),
    #[cfg(not(unix))]
    readonly: bool,
}

impl Located {
    /// Locate `path` with no root: what the library functions use.
    /// The path is canonicalised, so every symlink on it is followed.
    pub(crate) fn unconfined(path: &Path) -> io::Result<Self> {
        Self::from_path(&path.canonicalize()?)
    }

    fn from_path(canonical: &Path) -> io::Result<Self> {
        let name = canonical
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path names no file"))?;
        let parent = canonical.parent().unwrap_or(Path::new("."));
        Ok(Self {
            #[cfg(unix)]
            dir: std::os::fd::OwnedFd::from(std::fs::File::open(parent)?),
            #[cfg(not(unix))]
            dir: parent.to_path_buf(),
            name: name.to_os_string(),
        })
    }

    /// Read the whole file as UTF-8, with what a replacement keeps.
    pub(crate) fn read(&self) -> io::Result<(String, Kept)> {
        let mut file = self.open_read()?;
        let meta = file.metadata()?;
        let mut text = String::new();
        let _ = file.read_to_string(&mut text)?;
        Ok((text, kept(&meta)))
    }

    #[cfg(unix)]
    fn open_read(&self) -> io::Result<std::fs::File> {
        use rustix::fs::{Mode, OFlags};
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = rustix::fs::openat(&self.dir, &self.name, flags, Mode::empty())?;
        Ok(std::fs::File::from(fd))
    }

    #[cfg(not(unix))]
    fn open_read(&self) -> io::Result<std::fs::File> {
        std::fs::File::open(self.dir.join(&self.name))
    }

    /// Replace the file with `bytes`: write a new file beside it and
    /// rename it over the old one, so a reader sees one or the other
    /// and never a half-written file.
    ///
    /// The new file is created exclusively under a random name (a file
    /// or symlink already there is never opened), takes the old file's
    /// permissions before any byte is written, and is synced before
    /// the rename.
    pub(crate) fn replace(&self, bytes: &[u8], kept: Kept) -> io::Result<()> {
        self.replace_named(bytes, kept, &mut || temp_name(&self.name))
    }

    /// [`Located::replace`] with the temp names drawn from `names`, so
    /// a test can make the first one collide.
    pub(crate) fn replace_named(
        &self,
        bytes: &[u8],
        kept: Kept,
        names: &mut dyn FnMut() -> OsString,
    ) -> io::Result<()> {
        let (mut file, tmp) = self.create_temp(kept, names)?;
        let written = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .and_then(|()| self.rename_over(&tmp));
        if written.is_err() {
            self.remove(&tmp);
        }
        written
    }

    fn create_temp(
        &self,
        kept: Kept,
        names: &mut dyn FnMut() -> OsString,
    ) -> io::Result<(std::fs::File, OsString)> {
        let mut last = None;
        for _ in 0..TEMP_ATTEMPTS {
            let tmp = names();
            match self.create_exclusive(&tmp) {
                Ok(file) => {
                    if let Err(e) = apply(&file, kept) {
                        self.remove(&tmp);
                        return Err(e);
                    }
                    return Ok((file, tmp));
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
                Err(e) => return Err(e),
            }
        }
        Err(last.unwrap_or_else(|| io::Error::other("no temp name")))
    }

    #[cfg(unix)]
    fn create_exclusive(&self, tmp: &OsString) -> io::Result<std::fs::File> {
        use rustix::fs::{Mode, OFlags};
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let fd = rustix::fs::openat(&self.dir, tmp, flags, Mode::RUSR | Mode::WUSR)?;
        Ok(std::fs::File::from(fd))
    }

    #[cfg(not(unix))]
    fn create_exclusive(&self, tmp: &OsString) -> io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.dir.join(tmp))
    }

    #[cfg(unix)]
    fn rename_over(&self, tmp: &OsString) -> io::Result<()> {
        Ok(rustix::fs::renameat(&self.dir, tmp, &self.dir, &self.name)?)
    }

    #[cfg(not(unix))]
    fn rename_over(&self, tmp: &OsString) -> io::Result<()> {
        std::fs::rename(self.dir.join(tmp), self.dir.join(&self.name))
    }

    #[cfg(unix)]
    fn remove(&self, tmp: &OsString) {
        let _ = rustix::fs::unlinkat(&self.dir, tmp, rustix::fs::AtFlags::empty());
    }

    #[cfg(not(unix))]
    fn remove(&self, tmp: &OsString) {
        let _ = std::fs::remove_file(self.dir.join(tmp));
    }
}

/// `.{name}.{128 random bits}.tmp`: hidden, beside the target, and not
/// guessable by anyone who could plant something there.
fn temp_name(name: &OsString) -> OsString {
    let mut tmp = OsString::from(".");
    tmp.push(name);
    tmp.push(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    tmp
}

#[cfg(unix)]
fn kept(meta: &std::fs::Metadata) -> Kept {
    use std::os::unix::fs::MetadataExt;
    Kept {
        // The permission bits only: set-id and sticky bits are not
        // carried onto a file this process wrote.
        mode: meta.mode() & 0o777,
        owner: (meta.uid(), meta.gid()),
    }
}

#[cfg(not(unix))]
fn kept(meta: &std::fs::Metadata) -> Kept {
    Kept {
        readonly: meta.permissions().readonly(),
    }
}

/// Give the new file the old one's permissions and, where the process
/// may, its owner and group. The rename keeps the new file's owner, so
/// without this a replaced file would change hands.
#[cfg(unix)]
fn apply(file: &std::fs::File, kept: Kept) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let (uid, gid) = kept.owner;
    // Changing the owner needs privilege; the group only membership.
    // Neither is required: a file this process may write but not
    // chown ends up owned by the process, as with any editor.
    if std::os::unix::fs::fchown(file, Some(uid), Some(gid)).is_err() {
        let _ = std::os::unix::fs::fchown(file, None, Some(gid));
    }
    file.set_permissions(std::fs::Permissions::from_mode(kept.mode))
}

#[cfg(not(unix))]
fn apply(file: &std::fs::File, kept: Kept) -> io::Result<()> {
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(kept.readonly);
    file.set_permissions(permissions)
}

#[cfg(unix)]
mod unix {
    use std::collections::VecDeque;
    use std::ffi::OsString;
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};

    use rustix::fs::{AtFlags, FileType, Mode, OFlags};

    use std::path::Component;

    use super::{FileError, Located};

    /// How many symlinks one walk may follow, as the kernel's own limit.
    const MAX_SYMLINKS: usize = 40;

    /// The components of `path` for the walk, `..` kept as a component.
    /// A root or prefix is outside by definition.
    pub(super) fn components(path: &Path) -> Result<Vec<OsString>, FileError> {
        let mut out = Vec::new();
        for c in path.components() {
            match c {
                Component::Normal(part) => out.push(part.to_os_string()),
                Component::ParentDir => out.push(OsString::from("..")),
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => return Err(FileError::Outside),
            }
        }
        Ok(out)
    }

    /// One walk: the directories entered so far (the root first) and
    /// the components still to take.
    struct Walk<'a> {
        canonical_root: &'a Path,
        dirs: Vec<OwnedFd>,
        queue: VecDeque<OsString>,
        symlinks: usize,
    }

    pub(super) fn walk(
        root: &OwnedFd,
        canonical_root: &Path,
        relative: &Path,
    ) -> Result<Located, FileError> {
        let start = open_dir(root, ".")?;
        let mut walk = Walk {
            canonical_root,
            dirs: vec![start],
            queue: components(relative)?.into(),
            symlinks: 0,
        };
        while let Some(part) = walk.queue.pop_front() {
            if let Some(found) = walk.step(part)? {
                return Ok(found);
            }
        }
        Err(FileError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the path names a directory, not a file",
        )))
    }

    impl Walk<'_> {
        fn top(&self) -> &OwnedFd {
            // The root is never popped: `..` from it is refused.
            &self.dirs[self.dirs.len() - 1]
        }

        /// Take one component. `Some` when it is the last one.
        fn step(&mut self, part: OsString) -> Result<Option<Located>, FileError> {
            if part == ".." {
                if self.dirs.len() == 1 {
                    return Err(FileError::Outside);
                }
                drop(self.dirs.pop());
                return Ok(None);
            }
            let stat = rustix::fs::statat(self.top(), &part, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)?;
            let kind = FileType::from_raw_mode(stat.st_mode);
            if kind == FileType::Symlink {
                self.follow(&part)?;
                return Ok(None);
            }
            if self.queue.is_empty() {
                let dir = open_dir(self.top(), ".")?;
                return Ok(Some(Located { dir, name: part }));
            }
            let next = open_dir(self.top(), &part)?;
            self.dirs.push(next);
            Ok(None)
        }

        /// Put a symlink's target in front of the components still to
        /// take. An absolute target is followed only into the root.
        fn follow(&mut self, part: &OsString) -> Result<(), FileError> {
            self.symlinks += 1;
            if self.symlinks > MAX_SYMLINKS {
                return Err(FileError::Io(std::io::Error::from(rustix::io::Errno::LOOP)));
            }
            let target = rustix::fs::readlinkat(self.top(), part, Vec::new())
                .map_err(std::io::Error::from)?;
            let target = PathBuf::from(OsString::from_vec(target.into_bytes()));
            let rest = if target.is_absolute() {
                let inside = target
                    .strip_prefix(self.canonical_root)
                    .map_err(|_| FileError::Outside)?;
                self.dirs.truncate(1);
                components(inside)?
            } else {
                components(&target)?
            };
            for c in rest.into_iter().rev() {
                self.queue.push_front(c);
            }
            Ok(())
        }
    }

    fn open_dir(at: &OwnedFd, name: impl rustix::path::Arg) -> Result<OwnedFd, FileError> {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        rustix::fs::openat(at, name, flags, Mode::empty())
            .map_err(|e| FileError::Io(std::io::Error::from(e)))
    }
}

#[cfg(test)]
mod tests {
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
        Ok(at.read()?.0)
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
        let (_, kept) = at.read().unwrap();
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
        let (_, kept) = at.read().unwrap();
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
}
