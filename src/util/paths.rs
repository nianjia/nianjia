use std::env;
use std::iter;
use std::path::{Path, PathBuf};

use crate::util::errors::NianjiaResult;

pub fn ancestors(path: &Path) -> PathAncestors<'_> {
    PathAncestors::new(path)
}

pub struct PathAncestors<'a> {
    current: Option<&'a Path>,
    stop_at: Option<PathBuf>,
}

impl<'a> PathAncestors<'a> {
    fn new(path: &Path) -> PathAncestors<'_> {
        PathAncestors {
            current: Some(path),
            //HACK: avoid reading `~/.nianjia/config` when testing Nianjia itself.
            stop_at: env::var("__NIANJIA_TEST_ROOT").ok().map(PathBuf::from),
        }
    }
}

impl<'a> Iterator for PathAncestors<'a> {
    type Item = &'a Path;

    fn next(&mut self) -> Option<&'a Path> {
        if let Some(path) = self.current {
            self.current = path.parent();

            if let Some(ref stop_at) = self.stop_at {
                if path == stop_at {
                    self.current = None;
                }
            }

            Some(path)
        } else {
            None
        }
    }
}

pub fn resolve_executable(exec: &Path) -> NianjiaResult<PathBuf> {
    if exec.components().count() == 1 {
        let paths = env::var_os("PATH").ok_or_else(|| failure::format_err!("no PATH"))?;
        let candidates = env::split_paths(&paths).flat_map(|path| {
            let candidate = path.join(&exec);
            let with_exe = if env::consts::EXE_EXTENSION == "" {
                None
            } else {
                Some(candidate.with_extension(env::consts::EXE_EXTENSION))
            };
            iter::once(candidate).chain(with_exe)
        });
        for candidate in candidates {
            if candidate.is_file() {
                // PATH may have a component like "." in it, so we still need to
                // canonicalize.
                return Ok(candidate.canonicalize()?);
            }
        }

        failure::bail!("no executable for `{}` found in PATH", exec.display())
    } else {
        Ok(exec.canonicalize()?)
    }
}