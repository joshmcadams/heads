use heads::{process, repo::Status, report, scan, sync_repos};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

const USAGE: &str = "Usage: heads [--main] [--parallel N] [--timeout SECONDS] [folder]\n\nFast-forward clean Git repositories under folder (default: current directory).\n  --main             Switch clean repositories to their default branch\n  --parallel N       Maximum concurrent repositories (default: 5)\n  --timeout SECONDS  Per-repository timeout (default: 120)\n  -h, --help         Show this help\n  -V, --version      Show the version\n";

struct Options {
    root: PathBuf,
    main: bool,
    parallel: usize,
    timeout: Duration,
}

enum Action {
    Run(Options),
    Help,
    Version,
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let mut args = args.into_iter();
    let mut root = None;
    let mut main = false;
    let mut parallel = 5;
    let mut timeout = Duration::from_secs(120);
    let mut positional = false;
    while let Some(arg) = args.next() {
        if !positional {
            match arg.to_str() {
                Some("-h" | "--help") => return Ok(Action::Help),
                Some("-V" | "--version") => return Ok(Action::Version),
                Some("--") => {
                    positional = true;
                    continue;
                }
                Some("--main") => {
                    main = true;
                    continue;
                }
                Some("--parallel" | "--timeout") => {
                    let name = arg.to_string_lossy();
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{name} requires a value"))?;
                    let number = value
                        .to_str()
                        .and_then(|value| value.parse::<u64>().ok())
                        .filter(|number| *number > 0)
                        .ok_or_else(|| format!("{name} must be a positive integer"))?;
                    if name == "--parallel" {
                        parallel =
                            usize::try_from(number).map_err(|_| "--parallel is too large")?;
                    } else {
                        timeout = Duration::from_secs(number);
                        if std::time::Instant::now().checked_add(timeout).is_none() {
                            return Err("--timeout is too large".into());
                        }
                    }
                    continue;
                }
                Some(value) if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}"))
                }
                _ => {}
            }
        }
        if root.replace(PathBuf::from(arg)).is_some() {
            return Err("only one folder may be specified".into());
        }
    }
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let root = root
        .canonicalize()
        .map_err(|error| format!("{}: {error}", root.display()))?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    Ok(Action::Run(Options {
        root,
        main,
        parallel,
        timeout,
    }))
}

fn run() -> Result<i32, String> {
    let options = match parse(std::env::args_os().skip(1)) {
        Ok(Action::Run(options)) => options,
        Ok(Action::Help) => {
            print!("{USAGE}");
            return Ok(0);
        }
        Ok(Action::Version) => {
            println!("heads {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        Err(message) => {
            eprintln!("heads: {message}\n{USAGE}");
            return Ok(2);
        }
    };
    process::install_interrupt_handler().map_err(|error| error.to_string())?;
    let cancellation = process::Cancellation::default();
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "Scanning {}…", options.root.display()).map_err(|error| error.to_string())?;
    let found = scan::discover_until(&options.root, || cancellation.is_cancelled());
    writeln!(stderr, "Found {} repositories.", found.repos.len())
        .map_err(|error| error.to_string())?;
    let tty = stderr.is_terminal();
    let mut progress = report::Progress::new(&mut stderr, tty, found.repos.len());
    let results = sync_repos(
        &found.repos,
        options.main,
        options.parallel,
        options.timeout,
        cancellation.clone(),
        || {
            let _ = progress.advance();
        },
    );
    progress.finish().map_err(|error| error.to_string())?;
    if found.unreadable > 0 {
        writeln!(
            stderr,
            "Skipped {} unreadable directories.",
            found.unreadable
        )
        .map_err(|error| error.to_string())?;
    }
    io::stdout()
        .lock()
        .write_all(report::table(&options.root, &results).as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(if cancellation.is_cancelled() {
        130
    } else if results
        .iter()
        .any(|result| matches!(result.status, Status::Error(_)))
    {
        1
    } else {
        0
    })
}

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("heads: {message}");
            1
        }
    };
    std::process::exit(code);
}
