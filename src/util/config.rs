use crate::util::errors::internal;
use std::collections::hash_map::Entry::Occupied;
use std::collections::hash_map::Entry::Vacant;
use std::str::FromStr;
use std::fmt;
use std::mem;
use std::env;
use std::io::Read;
use std::fs::{self, File};
use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use std::cell::{RefCell, RefMut};

use lazycell::LazyCell;

use crate::util::toml as nianjia_toml;
use crate::util::flock::Filesystem;
use crate::core::shell::{Verbosity, Shell};
use crate::util::paths;
use crate::util::errors::{NianjiaResult, NianjiaResultExt};

use self::ConfigValue as CV;

/// Configuration information for nianjias. This is not specific to a build, it is information
/// relating to nianjia itself.
///
/// This struct implements `Default`: all fields can be inferred.
#[derive(Debug)]
pub struct Config {
    /// The location of the user's 'home' directory. OS-dependent.
    home_path: Filesystem,
    /// Information about how to write messages to the shell
    shell: RefCell<Shell>,
    /// A collection of configuration options
    values: LazyCell<HashMap<String, ConfigValue>>,
    /// The current working directory of nianjia
    cwd: PathBuf,
    /// The location of the nianjia executable (path to current process)
    nianjia_exe: LazyCell<PathBuf>,
    /// Environment variables, separated to assist testing.
    env: HashMap<String, String>,
}

impl Config {
    pub fn new(shell: Shell, cwd: PathBuf, home_path: Filesystem) -> Config {
        let env: HashMap<_, _> = env::vars_os()
            .filter_map(|(k, v)| {
                // Ignore any key/values that are not valid Unicode.
                match (k.into_string(), v.into_string()) {
                    (Ok(k), Ok(v)) => Some((k, v)),
                    _ => None,
                }
            })
            .collect();
        Config {
            home_path: home_path,
            shell: RefCell::new(shell),
            values: LazyCell::new(),
            cwd,
            nianjia_exe: LazyCell::new(),
            env
        }
    }

    pub fn default() -> NianjiaResult<Config> {
        let shell = Shell::new();
        let cwd =
            env::current_dir().chain_err(|| "couldn't get the current directory of the process")?;
        let home_path = homedir().ok_or_else(|| {
            failure::format_err!(
                "Nianjia couldn't find your home directory. \
                 This probably means that $HOME was not set."
            )
        })?;
        Ok(Config::new(shell, cwd, home_path))
    }

    /// Gets the user's Nianjia home directory (OS-dependent).
    pub fn home(&self) -> &Filesystem {
        &self.home_path
    }

    /// Gets a reference to the shell, e.g., for writing error messages.
    pub fn shell(&self) -> RefMut<'_, Shell> {
        self.shell.borrow_mut()
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    fn get_env<T>(&self, key: &ConfigKey) -> Result<OptValue<T>, ConfigError>
    where
        T: FromStr,
        <T as FromStr>::Err: fmt::Display,
    {
        let key = key.to_env();
        match self.env.get(&key) {
            Some(value) => {
                let definition = Definition::Environment(key);
                Ok(Some(Value {
                    val: value
                        .parse()
                        .map_err(|e| ConfigError::new(format!("{}", e), definition.clone()))?,
                    definition,
                }))
            }
            None => Ok(None),
        }
    }


    fn get_cv(&self, key: &str) -> NianjiaResult<Option<ConfigValue>> {
        let vals = self.values()?;
        let mut parts = key.split('.').enumerate();
        let mut val = match vals.get(parts.next().unwrap().1) {
            Some(val) => val,
            None => return Ok(None),
        };
        for (i, part) in parts {
            match *val {
                CV::Table(ref map, _) => {
                    val = match map.get(part) {
                        Some(val) => val,
                        None => return Ok(None),
                    }
                }
                CV::Integer(_, ref path)
                | CV::String(_, ref path)
                | CV::List(_, ref path)
                | CV::Boolean(_, ref path) => {
                    let idx = key.split('.').take(i).fold(0, |n, s| n + s.len()) + i - 1;
                    let key_so_far = &key[..idx];
                    failure::bail!(
                        "expected table for configuration key `{}`, \
                         but found {} in {}",
                        key_so_far,
                        val.desc(),
                        path.display()
                    )
                }
            }
        }
        Ok(Some(val.clone()))
    }

    pub fn values(&self) -> NianjiaResult<&HashMap<String, ConfigValue>> {
        self.values.try_borrow_with(|| self.load_values())
    }

    pub fn get_bool(&self, key: &str) -> NianjiaResult<OptValue<bool>> {
        self.get_bool_priv(&ConfigKey::from_str(key))
            .map_err(|e| e.into())
    }

    fn get_bool_priv(&self, key: &ConfigKey) -> Result<OptValue<bool>, ConfigError> {
        match self.get_env(key)? {
            Some(v) => Ok(Some(v)),
            None => {
                let config_key = key.to_config();
                let o_cv = self.get_cv(&config_key)?;
                match o_cv {
                    Some(CV::Boolean(b, path)) => Ok(Some(Value {
                        val: b,
                        definition: Definition::Path(path),
                    })),
                    Some(cv) => Err(ConfigError::expected(&config_key, "true/false", &cv)),
                    None => Ok(None),
                }
            }
        }
    }

    fn expected<T>(&self, ty: &str, key: &str, val: &CV) -> NianjiaResult<T> {
        val.expected(ty, key)
            .map_err(|e| failure::format_err!("invalid configuration for key `{}`\n{}", key, e))
    }


    // NOTE: this does **not** support environment variables. Use `get` instead
    // if you want that.
    pub fn get_list(&self, key: &str) -> NianjiaResult<OptValue<Vec<(String, PathBuf)>>> {
        match self.get_cv(key)? {
            Some(CV::List(i, path)) => Ok(Some(Value {
                val: i,
                definition: Definition::Path(path),
            })),
            Some(val) => self.expected("list", key, &val),
            None => Ok(None),
        }
    }


    pub fn get_string(&self, key: &str) -> NianjiaResult<OptValue<String>> {
        self.get_string_priv(&ConfigKey::from_str(key))
            .map_err(|e| e.into())
    }

    fn get_string_priv(&self, key: &ConfigKey) -> Result<OptValue<String>, ConfigError> {
        match self.get_env(key)? {
            Some(v) => Ok(Some(v)),
            None => {
                let config_key = key.to_config();
                let o_cv = self.get_cv(&config_key)?;
                match o_cv {
                    Some(CV::String(s, path)) => Ok(Some(Value {
                        val: s,
                        definition: Definition::Path(path),
                    })),
                    Some(cv) => Err(ConfigError::expected(&config_key, "a string", &cv)),
                    None => Ok(None),
                }
            }
        }
    }

     /// Loads configuration from the filesystem.
    pub fn load_values(&self) -> NianjiaResult<HashMap<String, ConfigValue>> {
        self.load_values_from(&self.cwd)
    }

    fn load_values_from(&self, path: &Path) -> NianjiaResult<HashMap<String, ConfigValue>> {
        let mut cfg = CV::Table(HashMap::new(), PathBuf::from("."));
        let home = self.home_path.clone().into_path_unlocked();

        walk_tree(path, &home, |path| {
            let mut contents = String::new();
            let mut file = File::open(&path)?;
            file.read_to_string(&mut contents)
                .chain_err(|| format!("failed to read configuration file `{}`", path.display()))?;
            let toml = nianjia_toml::parse(&contents, path, self).chain_err(|| {
                format!("could not parse TOML configuration in `{}`", path.display())
            })?;
            let value = CV::from_toml(path, toml).chain_err(|| {
                format!(
                    "failed to load TOML configuration from `{}`",
                    path.display()
                )
            })?;
            cfg.merge(value)
                .chain_err(|| format!("failed to merge configuration at `{}`", path.display()))?;
            Ok(())
        })
        .chain_err(|| "could not load Nianjia configuration")?;

        self.load_credentials(&mut cfg)?;
        match cfg {
            CV::Table(map, _) => Ok(map),
            _ => unreachable!(),
        }
    }


    /// Loads credentials config from the credentials file into the `ConfigValue` object, if
    /// present.
    fn load_credentials(&self, cfg: &mut ConfigValue) -> NianjiaResult<()> {
        let home_path = self.home_path.clone().into_path_unlocked();
        let credentials = home_path.join("credentials");
        if fs::metadata(&credentials).is_err() {
            return Ok(());
        }

        let mut contents = String::new();
        let mut file = File::open(&credentials)?;
        file.read_to_string(&mut contents).chain_err(|| {
            format!(
                "failed to read configuration file `{}`",
                credentials.display()
            )
        })?;

        let toml = nianjia_toml::parse(&contents, &credentials, self).chain_err(|| {
            format!(
                "could not parse TOML configuration in `{}`",
                credentials.display()
            )
        })?;

        let mut value = CV::from_toml(&credentials, toml).chain_err(|| {
            format!(
                "failed to load TOML configuration from `{}`",
                credentials.display()
            )
        })?;

        // Backwards compatibility for old `.nianjia/credentials` layout.
        {
            let value = match value {
                CV::Table(ref mut value, _) => value,
                _ => unreachable!(),
            };

            if let Some(token) = value.remove("token") {
                if let Vacant(entry) = value.entry("registry".into()) {
                    let mut map = HashMap::new();
                    map.insert("token".into(), token);
                    let table = CV::Table(map, PathBuf::from("."));
                    entry.insert(table);
                }
            }
        }

        // We want value to override `cfg`, so swap these.
        mem::swap(cfg, &mut value);
        cfg.merge(value)?;

        Ok(())
    }

    /// Gets the path to the `nianjia` executable.
    pub fn nianjia_exe(&self) -> NianjiaResult<&Path> {
        self.nianjia_exe
            .try_borrow_with(|| {
                fn from_current_exe() -> NianjiaResult<PathBuf> {
                    // Try fetching the path to `nianjia` using `env::current_exe()`.
                    // The method varies per operating system and might fail; in particular,
                    // it depends on `/proc` being mounted on Linux, and some environments
                    // (like containers or chroots) may not have that available.
                    let exe = env::current_exe()?.canonicalize()?;
                    Ok(exe)
                }

                fn from_argv() -> NianjiaResult<PathBuf> {
                    // Grab `argv[0]` and attempt to resolve it to an absolute path.
                    // If `argv[0]` has one component, it must have come from a `PATH` lookup,
                    // so probe `PATH` in that case.
                    // Otherwise, it has multiple components and is either:
                    // - a relative path (e.g., `./nianjia`, `target/debug/nianjia`), or
                    // - an absolute path (e.g., `/usr/local/bin/nianjia`).
                    // In either case, `Path::canonicalize` will return the full absolute path
                    // to the target if it exists.
                    let argv0 = env::args_os()
                        .map(PathBuf::from)
                        .next()
                        .ok_or_else(|| failure::format_err!("no argv[0]"))?;
                    paths::resolve_executable(&argv0)
                }

                let exe = from_current_exe()
                    .or_else(|_| from_argv())
                    .chain_err(|| "couldn't get the path to nianjia executable")?;
                Ok(exe)
            })
            .map(AsRef::as_ref)
    }


    pub fn configure(
        &mut self,
        verbose: u32,
        quiet: Option<bool>,
        color: &Option<String>,
        frozen: bool,
        locked: bool,
        target_dir: &Option<PathBuf>,
        unstable_flags: &[String],
    ) -> NianjiaResult<()> {
        let extra_verbose = verbose >= 2;
        let verbose = if verbose == 0 { None } else { Some(true) };

        // Ignore errors in the configuration files.
        let cfg_verbose = self.get_bool("term.verbose").unwrap_or(None).map(|v| v.val);
        let cfg_color = self.get_string("term.color").unwrap_or(None).map(|v| v.val);

        let color = color.as_ref().or_else(|| cfg_color.as_ref());

        let verbosity = match (verbose, cfg_verbose, quiet) {
            (Some(true), _, None) | (None, Some(true), None) => Verbosity::Verbose,

            // Command line takes precedence over configuration, so ignore the
            // configuration..
            (None, _, Some(true)) => Verbosity::Quiet,

            // Can't pass both at the same time on the command line regardless
            // of configuration.
            (Some(true), _, Some(true)) => {
                failure::bail!("cannot set both --verbose and --quiet");
            }

            // Can't actually get `Some(false)` as a value from the command
            // line, so just ignore them here to appease exhaustiveness checking
            // in match statements.
            (Some(false), _, _)
            | (_, _, Some(false))
            | (None, Some(false), None)
            | (None, None, None) => Verbosity::Normal,
        };

        let cli_target_dir = match target_dir.as_ref() {
            Some(dir) => Some(Filesystem::new(dir.clone())),
            None => None,
        };

        self.shell().set_verbosity(verbosity);
        self.shell().set_color_choice(color.map(|s| &s[..]))?;
        // self.extra_verbose = extra_verbose;
        // self.frozen = frozen;
        // self.locked = locked;
        // self.target_dir = cli_target_dir;
        // self.cli_flags.parse(unstable_flags)?;

        Ok(())
    }
}

pub fn homedir() -> Option<Filesystem> {
    Some(Filesystem::new(dirs::home_dir()?))
}

/// A segment of a config key.
///
/// Config keys are split on dots for regular keys, or underscores for
/// environment keys.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum ConfigKeyPart {
    /// Case-insensitive part (checks uppercase in environment keys).
    Part(String),
    /// Case-sensitive part (environment keys must match exactly).
    CasePart(String),
}

impl ConfigKeyPart {
    fn to_env(&self) -> String {
        match self {
            ConfigKeyPart::Part(s) => s.replace("-", "_").to_uppercase(),
            ConfigKeyPart::CasePart(s) => s.clone(),
        }
    }
        
    fn to_config(&self) -> String {
        match self {
            ConfigKeyPart::Part(s) => s.clone(),
            ConfigKeyPart::CasePart(s) => s.clone(),
        }
    }
}

/// Key for a configuration variable.
#[derive(Debug, Clone)]
struct ConfigKey(Vec<ConfigKeyPart>);

impl ConfigKey {
    fn from_str(key: &str) -> ConfigKey {
        ConfigKey(
            key.split('.')
                .map(|p| ConfigKeyPart::Part(p.to_string()))
                .collect(),
        )
    }

    fn to_env(&self) -> String {
        format!(
            "NIANJIA_{}",
            self.0
                .iter()
                .map(|p| p.to_env())
                .collect::<Vec<_>>()
                .join("_")
        )
    }

    fn to_config(&self) -> String {
        self.0
            .iter()
            .map(|p| p.to_config())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Eq, PartialEq, Clone)]
pub enum ConfigValue {
    Integer(i64, PathBuf),
    String(String, PathBuf),
    List(Vec<(String, PathBuf)>, PathBuf),
    Table(HashMap<String, ConfigValue>, PathBuf),
    Boolean(bool, PathBuf),
}

impl ConfigValue {
    fn from_toml(path: &Path, toml: toml::Value) -> NianjiaResult<ConfigValue> {
        match toml {
            toml::Value::String(val) => Ok(CV::String(val, path.to_path_buf())),
            toml::Value::Boolean(b) => Ok(CV::Boolean(b, path.to_path_buf())),
            toml::Value::Integer(i) => Ok(CV::Integer(i, path.to_path_buf())),
            toml::Value::Array(val) => Ok(CV::List(
                val.into_iter()
                    .map(|toml| match toml {
                        toml::Value::String(val) => Ok((val, path.to_path_buf())),
                        v => failure::bail!("expected string but found {} in list", v.type_str()),
                    })
                    .collect::<NianjiaResult<_>>()?,
                path.to_path_buf(),
            )),
            toml::Value::Table(val) => Ok(CV::Table(
                val.into_iter()
                    .map(|(key, value)| {
                        let value = CV::from_toml(path, value)
                            .chain_err(|| format!("failed to parse key `{}`", key))?;
                        Ok((key, value))
                    })
                    .collect::<NianjiaResult<_>>()?,
                path.to_path_buf(),
            )),
            v => failure::bail!(
                "found TOML configuration value of unknown type `{}`",
                v.type_str()
            ),
        }
    }

    pub fn definition_path(&self) -> &Path {
        match *self {
            CV::Boolean(_, ref p)
            | CV::Integer(_, ref p)
            | CV::String(_, ref p)
            | CV::List(_, ref p)
            | CV::Table(_, ref p) => p,
        }
    }


    fn merge(&mut self, from: ConfigValue) -> NianjiaResult<()> {
        match (self, from) {
            (&mut CV::List(ref mut old, _), CV::List(ref mut new, _)) => {
                let new = mem::replace(new, Vec::new());
                old.extend(new.into_iter());
            }
            (&mut CV::Table(ref mut old, _), CV::Table(ref mut new, _)) => {
                let new = mem::replace(new, HashMap::new());
                for (key, value) in new {
                    match old.entry(key.clone()) {
                        Occupied(mut entry) => {
                            let path = value.definition_path().to_path_buf();
                            let entry = entry.get_mut();
                            entry.merge(value).chain_err(|| {
                                format!(
                                    "failed to merge key `{}` between \
                                     files:\n  \
                                     file 1: {}\n  \
                                     file 2: {}",
                                    key,
                                    entry.definition_path().display(),
                                    path.display()
                                )
                            })?;
                        }
                        Vacant(entry) => {
                            entry.insert(value);
                        }
                    };
                }
            }
            // Allow switching types except for tables or arrays.
            (expected @ &mut CV::List(_, _), found)
            | (expected @ &mut CV::Table(_, _), found)
            | (expected, found @ CV::List(_, _))
            | (expected, found @ CV::Table(_, _)) => {
                return Err(internal(format!(
                    "expected {}, but found {}",
                    expected.desc(),
                    found.desc()
                )));
            }
            _ => {}
        }

        Ok(())
    }

    fn expected<T>(&self, wanted: &str, key: &str) -> NianjiaResult<T> {
        failure::bail!(
            "expected a {}, but found a {} for `{}` in {}",
            wanted,
            self.desc(),
            key,
            self.definition_path().display()
        )
    }
}

pub struct Value<T> {
    pub val: T,
    pub definition: Definition,
}

pub type OptValue<T> = Option<Value<T>>;

#[derive(Clone, Debug)]
pub enum Definition {
    Path(PathBuf),
    Environment(String),
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            CV::Integer(i, ref path) => write!(f, "{} (from {})", i, path.display()),
            CV::Boolean(b, ref path) => write!(f, "{} (from {})", b, path.display()),
            CV::String(ref s, ref path) => write!(f, "{} (from {})", s, path.display()),
            CV::List(ref list, ref path) => {
                write!(f, "[")?;
                for (i, &(ref s, ref path)) in list.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} (from {})", s, path.display())?;
                }
                write!(f, "] (from {})", path.display())
            }
            CV::Table(ref table, _) => write!(f, "{:?}", table),
        }
    }
}

impl ConfigValue {
    pub fn desc(&self) -> &'static str {
        match *self {
            CV::Table(..) => "table",
            CV::List(..) => "array",
            CV::String(..) => "string",
            CV::Boolean(..) => "boolean",
            CV::Integer(..) => "integer",
        }
    }
}

/// Internal error for serde errors.
#[derive(Debug)]
pub struct ConfigError {
    error: failure::Error,
    definition: Option<Definition>,
}

impl std::error::Error for ConfigError {}

impl ConfigError {
    fn new(message: String, definition: Definition) -> ConfigError {
        ConfigError {
            error: failure::err_msg(message),
            definition: Some(definition),
        }
    }

    fn expected(key: &str, expected: &str, found: &ConfigValue) -> ConfigError {
        ConfigError {
            error: failure::format_err!(
                "`{}` expected {}, but found a {}",
                key,
                expected,
                found.desc()
            ),
            definition: Some(Definition::Path(found.definition_path().to_path_buf())),
        }
    }
}

// Future note: currently, we cannot override `Fail::cause` (due to
// specialization) so we have no way to return the underlying causes. In the
// future, once this limitation is lifted, this should instead implement
// `cause` and avoid doing the cause formatting here.
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = self
            .error
            .iter_chain()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\nCaused by:\n  ");
        if let Some(ref definition) = self.definition {
            write!(f, "error in {}: {}", definition, message)
        } else {
            message.fmt(f)
        }
    }
}

impl From<failure::Error> for ConfigError {
    fn from(error: failure::Error) -> Self {
        ConfigError {
            error,
            definition: None,
        }
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Definition::Path(ref p) => p.display().fmt(f),
            Definition::Environment(ref key) => write!(f, "environment variable `{}`", key),
        }
    }
}

fn walk_tree<F>(pwd: &Path, home: &Path, mut walk: F) -> NianjiaResult<()>
where
    F: FnMut(&Path) -> NianjiaResult<()>,
{
    let mut stash: HashSet<PathBuf> = HashSet::new();

    for current in paths::ancestors(pwd) {
        let possible = current.join(".nianjia").join("config");
        if fs::metadata(&possible).is_ok() {
            walk(&possible)?;
            stash.insert(possible);
        }
    }

    // Once we're done, also be sure to walk the home directory even if it's not
    // in our history to be sure we pick up that standard location for
    // information.
    let config = home.join("config");
    if !stash.contains(&config) && fs::metadata(&config).is_ok() {
        walk(&config)?;
    }

    Ok(())
}
