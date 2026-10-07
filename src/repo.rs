use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Synced,
    UpToDate,
    Dirty,
    Detached,
    NoUpstream,
    Error(String),
    Cancelled,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Synced => f.write_str("synced"),
            Self::UpToDate => f.write_str("up-to-date"),
            Self::Dirty => f.write_str("dirty (skipped)"),
            Self::Detached => f.write_str("detached (skipped)"),
            Self::NoUpstream => f.write_str("no upstream (skipped)"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Error(message) => write!(f, "error: {}", collapse(message)),
        }
    }
}

pub fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Debug)]
pub struct RepoResult {
    pub path: PathBuf,
    pub branch: String,
    pub status: Status,
}

pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub enum RunError {
    Failed(String),
    Cancelled,
}

pub trait Runner {
    fn run(&mut self, args: &[&str]) -> Result<Output, RunError>;
}

pub fn command_error(args: &[&str], output: &Output) -> RunError {
    RunError::Failed(format!(
        "git {} (exit {}): {} {}",
        args.join(" "),
        output.code,
        output.stdout,
        output.stderr
    ))
}

fn checked(runner: &mut impl Runner, args: &[&str]) -> Result<String, RunError> {
    let output = runner.run(args)?;
    if output.code != 0 {
        return Err(command_error(args, &output));
    }
    Ok(output.stdout.trim().to_string())
}

pub fn process(path: PathBuf, main: bool, runner: &mut impl Runner) -> RepoResult {
    let mut branch = String::from("-");
    let outcome = procedure(main, runner, &mut branch);
    let status = match outcome {
        Ok(status) => status,
        Err(RunError::Failed(message)) => Status::Error(collapse(&message)),
        Err(RunError::Cancelled) => Status::Cancelled,
    };
    RepoResult {
        path,
        branch,
        status,
    }
}

fn procedure(
    main: bool,
    runner: &mut impl Runner,
    branch: &mut String,
) -> Result<Status, RunError> {
    let args = ["symbolic-ref", "--quiet", "--short", "HEAD"];
    let output = runner.run(&args)?;
    if output.code == 1 {
        return Ok(Status::Detached);
    }
    if output.code != 0 {
        return Err(command_error(&args, &output));
    }
    *branch = output.stdout.trim().to_string();
    checked(runner, &["rev-parse", "--verify", "HEAD"])?;
    if !checked(runner, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Ok(Status::Dirty);
    }
    if main {
        let args = [
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ];
        let default = runner.run(&args)?;
        let default = if default.code == 0 {
            default
                .stdout
                .trim()
                .strip_prefix("origin/")
                .ok_or_else(|| RunError::Failed("origin/HEAD does not reference origin".into()))?
                .to_string()
        } else if default.code == 1 {
            let mut found = None;
            for name in ["main", "master"] {
                let reference = format!("refs/heads/{name}");
                let args = ["show-ref", "--verify", "--quiet", &reference];
                let output = runner.run(&args)?;
                if output.code == 0 {
                    found = Some(name.to_string());
                    break;
                }
                if output.code != 1 {
                    return Err(command_error(&args, &output));
                }
            }
            found.ok_or_else(|| {
                RunError::Failed(
                    "cannot resolve default branch: no origin/HEAD, local main or master".into(),
                )
            })?
        } else {
            return Err(command_error(&args, &default));
        };
        let reference = format!("refs/heads/{default}");
        let args = ["show-ref", "--verify", "--quiet", &reference];
        let exists = runner.run(&args)?;
        if exists.code == 0 {
            checked(runner, &["switch", &default])?;
        } else if exists.code == 1 {
            let remote = format!("origin/{default}");
            checked(runner, &["switch", "--track", "-c", &default, &remote])?;
        } else {
            return Err(command_error(&args, &exists));
        }
        *branch = default;
    }
    let reference = format!("refs/heads/{branch}");
    if checked(
        runner,
        &["for-each-ref", "--format=%(upstream)", &reference],
    )?
    .is_empty()
    {
        return Ok(Status::NoUpstream);
    }
    let before = checked(runner, &["rev-parse", "--verify", "HEAD"])?;
    checked(runner, &["pull", "--ff-only", "--no-rebase"])?;
    let after = checked(runner, &["rev-parse", "--verify", "HEAD"])?;
    Ok(if before == after {
        Status::UpToDate
    } else {
        Status::Synced
    })
}
