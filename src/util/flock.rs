use std::path::{Path, PathBuf};

/// A "filesystem" is intended to be a globally shared, hence locked, resource
/// in Nianjia.
///
/// The `Path` of a filesystem cannot be learned unless it's done in a locked
/// fashion, and otherwise functions on this structure are prepared to handle
/// concurrent invocations across multiple instances of Nianjia.
#[derive(Clone, Debug)]
pub struct Filesystem {
    root: PathBuf,
}


impl Filesystem {
    /// Creates a new filesystem to be rooted at the given path.
    pub fn new(path: PathBuf) -> Filesystem {
        Filesystem { root: path }
    }

    /// Like `Path::join`, creates a new filesystem rooted at this filesystem
    /// joined with the given path.
    pub fn join<T: AsRef<Path>>(&self, other: T) -> Filesystem {
        Filesystem::new(self.root.join(other))
    }

    /// Like `Path::push`, pushes a new path component onto this filesystem.
    pub fn push<T: AsRef<Path>>(&mut self, other: T) {
        self.root.push(other);
    }

    /// Consumes this filesystem and returns the underlying `PathBuf`.
    ///
    /// Note that this is a relatively dangerous operation and should be used
    /// with great caution!.
    pub fn into_path_unlocked(self) -> PathBuf {
        self.root
    }
}