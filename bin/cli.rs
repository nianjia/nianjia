use clap::{AppSettings, ArgMatches};

use nianjia::util::config::Config;
use nianjia::util::errors::CliResult;
use nianjia::util::errors::CliError;

use super::commands;
use super::list_commands;
use nianjia::util::command_prelude::*;

pub fn main(config: &mut Config) -> CliResult {
    let args = match cli().get_matches_safe() {
        Ok(args) => args,
        Err(e) => {
            // if e.kind == clap::ErrorKind::UnrecognizedSubcommand {
            //     // An unrecognized subcommand might be an external subcommand.
            //     let cmd = &e.info.as_ref().unwrap()[0].to_owned();
            //     return super::execute_external_subcommand(config, cmd, &[cmd, "--help"])
            //         .map_err(|_| e.into());
            // } else {
                return Err(e)?;
            //}
        }
    };
    
    let is_verbose = args.occurrences_of("verbose") > 0;

    if args.is_present("list") {
        println!("Installed Commands:");
        for command in list_commands(config) {
            match command {
                CommandInfo::BuiltIn { name, about } => {
                    let summary = about.unwrap_or_default();
                    let summary = summary.lines().next().unwrap_or(&summary); // display only the first line
                    println!("    {:<20} {}", name, summary)
                }
                CommandInfo::External { name, path } => {
                    if is_verbose {
                        println!("    {:<20} {}", name, path.display())
                    } else {
                        println!("    {}", name)
                    }
                }
            }
        }
        return Ok(());
    }

    let args = expand_aliases(config, args)?;

    execute_subcommand(config, &args)
}

fn expand_aliases(
    config: &mut Config,
    args: ArgMatches<'static>,
) -> Result<ArgMatches<'static>, CliError> {
    if let (cmd, Some(args)) = args.subcommand() {
        match (
            commands::builtin_exec(cmd),
            super::aliased_command(config, cmd)?,
        ) {
            (Some(_), Some(_)) => {
                // User alias conflicts with a built-in subcommand
                config.shell().warn(format!(
                    "user-defined alias `{}` is ignored, because it is shadowed by a built-in command",
                    cmd,
                ))?;
            }
            (_, Some(mut alias)) => {
                alias.extend(
                    args.values_of("")
                        .unwrap_or_default()
                        .map(|s| s.to_string()),
                );
                let args = cli()
                    .setting(AppSettings::NoBinaryName)
                    .get_matches_from_safe(alias)?;
                return expand_aliases(config, args);
            }
            (_, None) => {}
        }
    };

    Ok(args)
}

fn execute_subcommand(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let (cmd, subcommand_args) = match args.subcommand() {
        (cmd, Some(args)) => (cmd, args),
        _ => {
            cli().print_help()?;
            return Ok(());
        }
    };

    let arg_target_dir = &subcommand_args.value_of_path("target-dir", config);

    config.configure(
        args.occurrences_of("verbose") as u32,
        if args.is_present("quiet") || subcommand_args.is_present("quiet") {
            Some(true)
        } else {
            None
        },
        &args.value_of("color").map(|s| s.to_string()),
        args.is_present("frozen"),
        args.is_present("locked"),
        arg_target_dir,
        &args
            .values_of_lossy("unstable-features")
            .unwrap_or_default(),
    )?;

    if let Some(exec) = commands::builtin_exec(cmd) {
        return exec(config, subcommand_args);
    }

    let mut ext_args: Vec<&str> = vec![cmd];
    ext_args.extend(subcommand_args.values_of("").unwrap_or_default());
    super::execute_external_subcommand(config, cmd, &ext_args)
}

fn cli() -> App {
  App::new("nianjia")
        .settings(&[
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::VersionlessSubcommands,
            AppSettings::AllowExternalSubcommands,
        ])
        .about("")
        .template(
            "\
Nianjia, the Sandboxing Environment for Next Generation Computation

USAGE:
    {usage}

OPTIONS:
{unified}

Some common nianjia commands are (see all commands with --list):
    run         Run a binary or example of the local package

See 'nianjia help <command>' for more information on a specific command.\n",
        )
        .arg(opt("version", "Print version info and exit").short("V"))
        .arg(opt("list", "List installed commands"))
        .arg(
            opt(
                "verbose",
                "Use verbose output (-vv very verbose output)",
            )
            .short("v")
            .multiple(true)
            .global(true),
        )
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(
            opt("color", "Coloring: auto, always, never")
                .value_name("WHEN")
                .global(true),
        )
        .subcommands(commands::builtin())
}