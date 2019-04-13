use std::fmt;
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

pub type CliResult = Result<(), CliError>;

#[derive(Debug)]
pub struct CliError {
    pub error: Option<failure::Error>,
    pub unknown: bool,
    pub exit_code: i32,
}

pub struct Internal {
    inner: Error,
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