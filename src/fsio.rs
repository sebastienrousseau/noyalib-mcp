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
    ///
    /// Only a regular file is read, and only when it is at most `limit`
    /// bytes: a FIFO or device would block or never end, and a file
    /// over the limit would be refused by the parser anyway after
    /// costing its size in memory. The open does not block (a FIFO
    /// opens at once and is then refused), and the read stops one byte
    /// past the limit in case the file grew after it was measured.
    pub(crate) fn read(&self, limit: usize) -> io::Result<(String, Kept)> {
        let file = self.open_read()?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        let too_big = |size: u64| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the file is {size} bytes, over the {limit}-byte document limit"),
            )
        };
        let cap = u64::try_from(limit).unwrap_or(u64::MAX);
        if meta.len() > cap {
            return Err(too_big(meta.len()));
        }
        let mut text = String::new();
        let read = file.take(cap.saturating_add(1)).read_to_string(&mut text)?;
        if read > limit {
            return Err(too_big(read as u64));
        }
        Ok((text, kept(&meta)))
    }

    #[cfg(unix)]
    fn open_read(&self) -> io::Result<std::fs::File> {
        use rustix::fs::{Mode, OFlags};
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let fd = rustix::fs::openat(&self.dir, &self.name, flags, Mode::empty())?;
        Ok(std::fs::File::from(fd))
    }

    #[cfg(not(unix))]
    fn open_read(&self) -> io::Result<std::fs::File> {
        // Windows refuses to open a directory with "Access is denied";
        // say what the Unix path says instead.
        let path = self.dir.join(&self.name);
        if std::fs::metadata(&path).is_ok_and(|m| !m.is_file()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        std::fs::File::open(path)
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
mod unix;

#[cfg(test)]
mod tests;
