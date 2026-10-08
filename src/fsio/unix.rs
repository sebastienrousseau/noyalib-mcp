// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The walk from the root handle, on unix.

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
        let target =
            rustix::fs::readlinkat(self.top(), part, Vec::new()).map_err(std::io::Error::from)?;
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
