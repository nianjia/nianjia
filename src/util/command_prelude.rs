use std::path::PathBuf;
use clap;
pub use clap::{AppSettings, Arg, ArgMatches};

use crate::util::config::Config;

pub type App = clap::App<'static, 'static>;

pub fn opt(name: &'static str, help: &'static str) -> Arg<'static, 'static> {
    Arg::with_name(name).long(name).help(help)
}

#[derive(PartialEq, PartialOrd, Eq, Ord)]
pub enum CommandInfo {
    BuiltIn { name: String, about: Option<String> },
    External { name: String, path: PathBuf },
}

impl CommandInfo {
    pub fn name(&self) -> String {
        match self {
            CommandInfo::BuiltIn { name, .. } => name.to_string(),
            CommandInfo::External { name, .. } => name.to_string(),
        }
    }
}

pub trait ArgMatchesExt {
    /// Returns value of the `name` command-line argument as an absolute path
    fn value_of_path(&self, name: &str, config: &Config) -> Option<PathBuf> {
        self._value_of(name).map(|path| config.cwd().join(path))
    }

    fn _value_of(&self, name: &str) -> Option<&str>;
}


impl<'a> ArgMatchesExt for ArgMatches<'a> {
    fn _value_of(&self, name: &str) -> Option<&str> {
        self.value_of(name)
    }
}
