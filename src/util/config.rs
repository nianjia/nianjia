use std::env;
use std::path::{Path, PathBuf};
use crate::core::shell::Shell;
use crate::util::errors::{NianjiaResult, NianjiaResultExt};

/// Configuration information for nianjias. This is not specific to a build, it is information
/// relating to nianjia itself.
///
/// This struct implements `Default`: all fields can be inferred.
#[derive(Debug)]
pub struct Config {
}

impl Config {
    pub fn new() -> Config {
        Config {}
    }

    pub fn default() -> NianjiaResult<Config> {
        let shell = Shell::new();
        let cwd =
            env::current_dir().chain_err(|| "couldn't get the current directory of the process")?;
        let homedir = homedir(&cwd).ok_or_else(|| {
            failure::format_err!(
                "Nianjia couldn't find your home directory. \
                 This probably means that $HOME was not set."
            )
        })?;
        Ok(Config::new())
    }
}

pub fn homedir(cwd: &Path) -> Option<PathBuf> {
    unimplemented!()
}
