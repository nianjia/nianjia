use std::fmt;
use std::io::prelude::Write;

use termcolor::{ColorSpec, StandardStream, WriteColor};
use termcolor::Color::{self, Red, Yellow};

use crate::util::errors::NianjiaResult;

/// An abstraction around a `Write`able object that remembers preferences for output verbosity and
/// color.
pub struct Shell {
    /// the `Write`able object, either with or without color support (represented by different enum
    /// variants)
    err: ShellOut,
    /// How verbose messages should be
    verbosity: Verbosity,
    /// Flag that indicates the current line needs to be cleared before
    /// printing. Used when a progress bar is currently displayed.
    needs_clear: bool,
}

impl fmt::Debug for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.err {
            ShellOut::Write(_) => f
                .debug_struct("Shell")
                .field("verbosity", &self.verbosity)
                .finish(),
            ShellOut::Stream { color_choice, .. } => f
                .debug_struct("Shell")
                .field("verbosity", &self.verbosity)
                .field("color_choice", &color_choice)
                .finish(),
        }
    }
}

/// The requested verbosity of output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

/// A `Write`able object, either with or without color support
enum ShellOut {
    /// A plain write object without color support
    Write(Box<dyn Write>),
    /// Color-enabled stdio, with information on whether color should be used
    Stream {
        stream: StandardStream,
        tty: bool,
        color_choice: ColorChoice,
    },
}

/// Whether messages should use color output
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ColorChoice {
    /// Force color output
    Always,
    /// Force disable color output
    Never,
    /// Intelligently guess whether to use color output
    NianjiaAuto,
}

impl Shell {
    /// Creates a new shell (color choice and verbosity), defaulting to 'auto' color and verbose
    /// output.
    pub fn new() -> Shell {
        Shell {
            err: ShellOut::Stream {
                stream: StandardStream::stderr(ColorChoice::NianjiaAuto.to_termcolor_color_choice()),
                color_choice: ColorChoice::NianjiaAuto,
                tty: atty::is(atty::Stream::Stderr),
            },
            verbosity: Verbosity::Verbose,
            needs_clear: false,
        }
    }

    /// Prints a message, where the status will have `color` color, and can be justified. The
    /// messages follows without color.
    fn print(
        &mut self,
        status: &dyn fmt::Display,
        message: Option<&dyn fmt::Display>,
        color: Color,
        justified: bool,
    ) -> NianjiaResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => {
                if self.needs_clear {
                    self.err_erase_line();
                }
                self.err.print(status, message, color, justified)
            }
        }
    }
        
    /// Gets a reference to the underlying writer.
    pub fn err(&mut self) -> &mut dyn Write {
        if self.needs_clear {
            self.err_erase_line();
        }
        self.err.as_write()
    }

    /// Erase from cursor to end of line.
    pub fn err_erase_line(&mut self) {
        if let ShellOut::Stream { tty: true, .. } = self.err {
            imp::err_erase_line(self);
            self.needs_clear = false;
        }
    }

    /// Prints a red 'error' message.
    pub fn error<T: fmt::Display>(&mut self, message: T) -> NianjiaResult<()> {
        self.print(&"error:", Some(&message), Red, false)
    }
    
    /// Gets the verbosity of the shell.
    pub fn verbosity(&self) -> Verbosity {
        self.verbosity
    }
    
    /// Prints an amber 'warning' message.
    pub fn warn<T: fmt::Display>(&mut self, message: T) -> NianjiaResult<()> {
        match self.verbosity {
            Verbosity::Quiet => Ok(()),
            _ => self.print(&"warning:", Some(&message), Yellow, false),
        }
    }

    /// Updates the verbosity of the shell.
    pub fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }


    /// Updates the color choice (always, never, or auto) from a string..
    pub fn set_color_choice(&mut self, color: Option<&str>) -> NianjiaResult<()> {
        if let ShellOut::Stream {
            ref mut stream,
            ref mut color_choice,
            ..
        } = self.err
        {
            let cfg = match color {
                Some("always") => ColorChoice::Always,
                Some("never") => ColorChoice::Never,

                Some("auto") | None => ColorChoice::NianjiaAuto,

                Some(arg) => failure::bail!(
                    "argument for --color must be auto, always, or \
                     never, but found `{}`",
                    arg
                ),
            };
            *color_choice = cfg;
            *stream = StandardStream::stderr(cfg.to_termcolor_color_choice());
        }
        Ok(())
    }
}

impl ShellOut {
    /// Prints out a message with a status. The status comes first, and is bold plus the given
    /// color. The status can be justified, in which case the max width that will right align is
    /// 12 chars.
    fn print(
        &mut self,
        status: &dyn fmt::Display,
        message: Option<&dyn fmt::Display>,
        color: Color,
        justified: bool,
    ) -> NianjiaResult<()> {
        match *self {
            ShellOut::Stream { ref mut stream, .. } => {
                stream.reset()?;
                stream.set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
                if justified {
                    write!(stream, "{:>12}", status)?;
                } else {
                    write!(stream, "{}", status)?;
                }
                stream.reset()?;
                match message {
                    Some(message) => writeln!(stream, " {}", message)?,
                    None => write!(stream, " ")?,
                }
            }
            ShellOut::Write(ref mut w) => {
                if justified {
                    write!(w, "{:>12}", status)?;
                } else {
                    write!(w, "{}", status)?;
                }
                match message {
                    Some(message) => writeln!(w, " {}", message)?,
                    None => write!(w, " ")?,
                }
            }
        }
        Ok(())
    }

    /// Gets this object as a `io::Write`.
    fn as_write(&mut self) -> &mut dyn Write {
        match *self {
            ShellOut::Stream { ref mut stream, .. } => stream,
            ShellOut::Write(ref mut w) => w,
        }
    }
}

impl ColorChoice {
    /// Converts our color choice to termcolor's version.
    fn to_termcolor_color_choice(self) -> termcolor::ColorChoice {
        match self {
            ColorChoice::Always => termcolor::ColorChoice::Always,
            ColorChoice::Never => termcolor::ColorChoice::Never,
            ColorChoice::NianjiaAuto => {
                if atty::is(atty::Stream::Stderr) {
                    termcolor::ColorChoice::Auto
                } else {
                    termcolor::ColorChoice::Never
                }
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod imp {
    use super::Shell;
    
    pub fn err_erase_line(shell: &mut Shell) {
        // This is the "EL - Erase in Line" sequence. It clears from the cursor
        // to the end of line.
        // https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_sequences
        let _ = shell.err.as_write().write_all(b"\x1B[K");
    }
}

#[cfg(windows)]
mod imp {
    use std::{cmp, mem, ptr};
    use winapi::um::fileapi::*;
    use winapi::um::handleapi::*;
    use winapi::um::processenv::*;
    use winapi::um::winbase::*;
    use winapi::um::wincon::*;
    use winapi::um::winnt::*;

    pub(super) use super::default_err_erase_line as err_erase_line;

    pub fn stderr_width() -> Option<usize> {
        unsafe {
            let stdout = GetStdHandle(STD_ERROR_HANDLE);
            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
            if GetConsoleScreenBufferInfo(stdout, &mut csbi) != 0 {
                return Some((csbi.srWindow.Right - csbi.srWindow.Left) as usize);
            }

            // On mintty/msys/cygwin based terminals, the above fails with
            // INVALID_HANDLE_VALUE. Use an alternate method which works
            // in that case as well.
            let h = CreateFileA(
                "CONOUT$\0".as_ptr() as *const CHAR,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
            let rc = GetConsoleScreenBufferInfo(h, &mut csbi);
            CloseHandle(h);
            if rc != 0 {
                let width = (csbi.srWindow.Right - csbi.srWindow.Left) as usize;
                // Unfortunately cygwin/mintty does not set the size of the
                // backing console to match the actual window size. This
                // always reports a size of 80 or 120 (not sure what
                // determines that). Use a conservative max of 60 which should
                // work in most circumstances. ConEmu does some magic to
                // resize the console correctly, but there's no reasonable way
                // to detect which kind of terminal we are running in, or if
                // GetConsoleScreenBufferInfo returns accurate information.
                return Some(cmp::min(60, width));
            }
            None
        }
    }
}

#[cfg(any(all(unix, not(any(target_os = "linux", target_os = "macos"))), windows,))]
fn default_err_erase_line(shell: &mut Shell) {
    if let Some(max_width) = imp::stderr_width() {
        let blank = " ".repeat(max_width);
        drop(write!(shell.err.as_write(), "{}\r", blank));
    }
}
