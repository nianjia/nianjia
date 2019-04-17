use std::fmt;
use std::str;
use std::process::{ExitStatus, Output};

use failure::{Error, Context, Fail};
use log::trace;

pub type NianjiaResult<T> = failure::Fallible<T>; 

pub trait NianjiaResultExt<T, E> {
    fn chain_err<F, D>(self, f: F) -> Result<T, Context<D>>
    where
        F: FnOnce() -> D,
        D: fmt::Display + Send + Sync + 'static;
}

impl<T, E> NianjiaResultExt<T, E> for Result<T, E>
where
    E: Into<Error>,
{
    fn chain_err<F, D>(self, f: F) -> Result<T, Context<D>>
    where
        F: FnOnce() -> D,
        D: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|failure| {
            let err = failure.into();
            let context = f();
            trace!("error: {}", err);
            trace!("\tcontext: {}", context);
            err.context(context)
        })
    }
}

pub struct Internal {
    inner: Error,
}

impl Internal {
    pub fn new(inner: Error) -> Internal {
        Internal { inner }
    }
}

pub type CliResult = Result<(), CliError>;

#[derive(Debug)]
pub struct CliError {
    pub error: Option<failure::Error>,
    pub unknown: bool,
    pub exit_code: i32,
}

impl Fail for Internal {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.as_fail().cause()
    }
}

impl fmt::Debug for Internal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}


impl CliError {
    pub fn new(error: failure::Error, code: i32) -> CliError {
        let unknown = error.downcast_ref::<Internal>().is_some();
        CliError {
            error: Some(error),
            exit_code: code,
            unknown,
        }
    }
    
    pub fn code(code: i32) -> CliError {
        CliError {
            error: None,
            exit_code: code,
            unknown: false,
        }
    }
}

impl fmt::Display for Internal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<failure::Error> for CliError {
    fn from(err: failure::Error) -> CliError {
        CliError::new(err, 101)
    }
}

impl From<clap::Error> for CliError {
    fn from(err: clap::Error) -> CliError {
        let code = if err.use_stderr() { 1 } else { 0 };
        CliError::new(err.into(), code)
    }
}

// =============================================================================
// Process errors
#[derive(Debug, Fail)]
#[fail(display = "{}", desc)]
pub struct ProcessError {
    pub desc: String,
    pub exit: Option<ExitStatus>,
    pub output: Option<Output>,
}

// =============================================================================
// Construction helpers

pub fn process_error(
    msg: &str,
    status: Option<ExitStatus>,
    output: Option<&Output>,
) -> ProcessError {
    let exit = match status {
        Some(s) => status_to_string(s),
        None => "never executed".to_string(),
    };
    let mut desc = format!("{} ({})", &msg, exit);

    if let Some(out) = output {
        match str::from_utf8(&out.stdout) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stdout\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
        match str::from_utf8(&out.stderr) {
            Ok(s) if !s.trim().is_empty() => {
                desc.push_str("\n--- stderr\n");
                desc.push_str(s);
            }
            Ok(..) | Err(..) => {}
        }
    }

    return ProcessError {
        desc,
        exit: status,
        output: output.cloned(),
    };

    #[cfg(unix)]
    fn status_to_string(status: ExitStatus) -> String {
        use std::os::unix::process::*;

        if let Some(signal) = status.signal() {
            let name = match signal as libc::c_int {
                libc::SIGABRT => ", SIGABRT: process abort signal",
                libc::SIGALRM => ", SIGALRM: alarm clock",
                libc::SIGFPE => ", SIGFPE: erroneous arithmetic operation",
                libc::SIGHUP => ", SIGHUP: hangup",
                libc::SIGILL => ", SIGILL: illegal instruction",
                libc::SIGINT => ", SIGINT: terminal interrupt signal",
                libc::SIGKILL => ", SIGKILL: kill",
                libc::SIGPIPE => ", SIGPIPE: write on a pipe with no one to read",
                libc::SIGQUIT => ", SIGQUIT: terminal quite signal",
                libc::SIGSEGV => ", SIGSEGV: invalid memory reference",
                libc::SIGTERM => ", SIGTERM: termination signal",
                libc::SIGBUS => ", SIGBUS: access to undefined memory",
                #[cfg(not(target_os = "haiku"))]
                libc::SIGSYS => ", SIGSYS: bad system call",
                libc::SIGTRAP => ", SIGTRAP: trace/breakpoint trap",
                _ => "",
            };
            format!("signal: {}{}", signal, name)
        } else {
            status.to_string()
        }
    }

    #[cfg(windows)]
    fn status_to_string(status: ExitStatus) -> String {
        use winapi::shared::minwindef::DWORD;
        use winapi::um::winnt::*;

        let mut base = status.to_string();
        let extra = match status.code().unwrap() as DWORD {
            STATUS_ACCESS_VIOLATION => "STATUS_ACCESS_VIOLATION",
            STATUS_IN_PAGE_ERROR => "STATUS_IN_PAGE_ERROR",
            STATUS_INVALID_HANDLE => "STATUS_INVALID_HANDLE",
            STATUS_INVALID_PARAMETER => "STATUS_INVALID_PARAMETER",
            STATUS_NO_MEMORY => "STATUS_NO_MEMORY",
            STATUS_ILLEGAL_INSTRUCTION => "STATUS_ILLEGAL_INSTRUCTION",
            STATUS_NONCONTINUABLE_EXCEPTION => "STATUS_NONCONTINUABLE_EXCEPTION",
            STATUS_INVALID_DISPOSITION => "STATUS_INVALID_DISPOSITION",
            STATUS_ARRAY_BOUNDS_EXCEEDED => "STATUS_ARRAY_BOUNDS_EXCEEDED",
            STATUS_FLOAT_DENORMAL_OPERAND => "STATUS_FLOAT_DENORMAL_OPERAND",
            STATUS_FLOAT_DIVIDE_BY_ZERO => "STATUS_FLOAT_DIVIDE_BY_ZERO",
            STATUS_FLOAT_INEXACT_RESULT => "STATUS_FLOAT_INEXACT_RESULT",
            STATUS_FLOAT_INVALID_OPERATION => "STATUS_FLOAT_INVALID_OPERATION",
            STATUS_FLOAT_OVERFLOW => "STATUS_FLOAT_OVERFLOW",
            STATUS_FLOAT_STACK_CHECK => "STATUS_FLOAT_STACK_CHECK",
            STATUS_FLOAT_UNDERFLOW => "STATUS_FLOAT_UNDERFLOW",
            STATUS_INTEGER_DIVIDE_BY_ZERO => "STATUS_INTEGER_DIVIDE_BY_ZERO",
            STATUS_INTEGER_OVERFLOW => "STATUS_INTEGER_OVERFLOW",
            STATUS_PRIVILEGED_INSTRUCTION => "STATUS_PRIVILEGED_INSTRUCTION",
            STATUS_STACK_OVERFLOW => "STATUS_STACK_OVERFLOW",
            STATUS_DLL_NOT_FOUND => "STATUS_DLL_NOT_FOUND",
            STATUS_ORDINAL_NOT_FOUND => "STATUS_ORDINAL_NOT_FOUND",
            STATUS_ENTRYPOINT_NOT_FOUND => "STATUS_ENTRYPOINT_NOT_FOUND",
            STATUS_CONTROL_C_EXIT => "STATUS_CONTROL_C_EXIT",
            STATUS_DLL_INIT_FAILED => "STATUS_DLL_INIT_FAILED",
            STATUS_FLOAT_MULTIPLE_FAULTS => "STATUS_FLOAT_MULTIPLE_FAULTS",
            STATUS_FLOAT_MULTIPLE_TRAPS => "STATUS_FLOAT_MULTIPLE_TRAPS",
            STATUS_REG_NAT_CONSUMPTION => "STATUS_REG_NAT_CONSUMPTION",
            STATUS_HEAP_CORRUPTION => "STATUS_HEAP_CORRUPTION",
            STATUS_STACK_BUFFER_OVERRUN => "STATUS_STACK_BUFFER_OVERRUN",
            STATUS_ASSERTION_FAILURE => "STATUS_ASSERTION_FAILURE",
            _ => return base,
        };
        base.push_str(", ");
        base.push_str(extra);
        base
    }
}

pub fn internal<S: fmt::Display>(error: S) -> failure::Error {
    _internal(&error)
}

fn _internal(error: &dyn fmt::Display) -> failure::Error {
    Internal::new(failure::format_err!("{}", error)).into()
}
