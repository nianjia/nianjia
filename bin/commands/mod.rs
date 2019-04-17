use nianjia::util::command_prelude::*;
use nianjia::util::config::Config;
use nianjia::util::errors::CliResult;
use clap::ArgMatches;

pub fn builtin() -> Vec<App> {
    vec![]
}

pub fn builtin_exec(cmd: &str) -> Option<fn(&mut Config, &ArgMatches<'_>) -> CliResult> {
    let f = match cmd {
        _ => return None,
    };
    Some(f)
}