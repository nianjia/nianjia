use nianjia::util::config::Config;
use nianjia::core::shell::Shell;

fn main() {
    #[cfg(feature = "pretty-env-logger")]
    pretty_env_logger::init();
    #[cfg(not(feature = "pretty-env-logger"))]
    env_logger::init(); 
    
    let mut config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            nianjia::exit_with_error(e.into(), &mut shell)
        }
    };
}