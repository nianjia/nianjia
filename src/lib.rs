pub mod core;
pub mod util;

use failure::Error;
use crate::core::shell::Verbosity::Verbose;
use log::debug;
use crate::core::shell::Shell;
pub use crate::util::errors::Internal;
pub use crate::util::errors::CliError;

pub fn exit_with_error(err: CliError, shell: &mut Shell) -> ! {
    debug!("exit_with_error; err={:?}", err);
    if let Some(ref err) = err.error {
        if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
            clap_err.exit()
        }
    }

    let CliError {
        error,
        exit_code,
        unknown,
    } = err;
    // `exit_code` of 0 means non-fatal error (e.g., docopt version info).
    let fatal = exit_code != 0;

    let hide = unknown && shell.verbosity() != Verbose;

    if let Some(error) = error {
        if hide {
            drop(shell.error("An unknown error occurred"))
        } else if fatal {
            drop(shell.error(&error))
        } else {
            println!("{}", error);
        }

        if !handle_cause(&error, shell) || hide {
            drop(writeln!(
                shell.err(),
                "\nTo learn more, run the command again \
                 with --verbose."
            ));
        }
    }

    std::process::exit(exit_code)
}


fn handle_cause(nianjia_err: &Error, shell: &mut Shell) -> bool {
    fn print(error: &str, shell: &mut Shell) {
        drop(writeln!(shell.err(), "\nCaused by:"));
        drop(writeln!(shell.err(), "  {}", error));
    }

    let verbose = shell.verbosity();

    if verbose == Verbose {
        // The first error has already been printed to the shell.
        // Print all remaining errors.
        for err in nianjia_err.iter_causes() {
            print(&err.to_string(), shell);
        }
    } else {
        // The first error has already been printed to the shell.
        // Print remaining errors until one marked as `Internal` appears.
        for err in nianjia_err.iter_causes() {
            if err.downcast_ref::<Internal>().is_some() {
                return false;
            }

            print(&err.to_string(), shell);
        }
    }

    true
}

pub const NIANJIA_ENV: &str = "NIANJIA";

