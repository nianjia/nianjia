use std::path::Path;

use serde::de::Deserialize;

use crate::util::config::Config;
use crate::util::errors::NianjiaResult;

pub fn parse(toml: &str, file: &Path, config: &Config) -> NianjiaResult<toml::Value> {
    let first_error = match toml.parse() {
        Ok(ret) => return Ok(ret),
        Err(e) => e,
    };

    let mut second_parser = toml::de::Deserializer::new(toml);
    second_parser.set_require_newline_after_table(false);
    if let Ok(ret) = toml::Value::deserialize(&mut second_parser) {
        let msg = format!(
            "\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires newlines after table definitions (e.g., `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.",
            file.display()
        );
        config.shell().warn(&msg)?;
        return Ok(ret);
    }

    let mut third_parser = toml::de::Deserializer::new(toml);
    third_parser.set_allow_duplicate_after_longer_table(true);
    if let Ok(ret) = toml::Value::deserialize(&mut third_parser) {
        let msg = format!(
            "\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires that each table header is defined at most once, but
historical versions of NIANJIA have erroneously accepted this file. The table
definitions will need to be merged together with one table header to proceed,
and this will become a hard error in the future.",
            file.display()
        );
        config.shell().warn(&msg)?;
        return Ok(ret);
    }

    let first_error = failure::Error::from(first_error);
    Err(first_error.context("could not parse input as TOML").into())
}
