#![allow(dead_code)]

use heads::process::{Cancellation, GitRunner};
use heads::repo::{self, RepoResult};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Temp(pub PathBuf);

impl Temp {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "heads-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn command(path: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("--no-optional-locks")
        .args(args)
        .current_dir(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .output()
        .unwrap()
}

pub fn git(path: &Path, args: &[&str]) -> String {
    let output = command(path, args);
    assert!(
        output.status.success(),
        "git {args:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub fn configure(path: &Path) {
    git(path, &["config", "user.name", "Heads Test"]);
    git(path, &["config", "user.email", "heads@example.invalid"]);
    git(path, &["config", "commit.gpgSign", "false"]);
    git(path, &["config", "core.hooksPath", "/dev/null"]);
    git(path, &["config", "core.excludesFile", "/dev/null"]);
    // The unreachable-loopback fixture must stay local even on machines with
    // configured HTTP proxies or interactive credential helpers.
    git(path, &["config", "http.proxy", ""]);
    git(path, &["config", "credential.helper", ""]);
}

pub struct Fixture {
    pub temp: Temp,
    pub remote: PathBuf,
    pub source: PathBuf,
    pub repo: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let temp = Temp::new();
        let remote = temp.0.join("remote.git");
        let source = temp.0.join("source");
        let repo = temp.0.join("repo");
        git(
            &temp.0,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ],
        );
        git(
            &temp.0,
            &["init", "--initial-branch=main", source.to_str().unwrap()],
        );
        configure(&source);
        fs::write(source.join("file.txt"), "initial\n").unwrap();
        git(&source, &["add", "file.txt"]);
        git(&source, &["commit", "-m", "initial"]);
        git(
            &source,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&source, &["push", "-u", "origin", "main"]);
        git(
            &temp.0,
            &["clone", remote.to_str().unwrap(), repo.to_str().unwrap()],
        );
        configure(&repo);
        Self {
            temp,
            remote,
            source,
            repo,
        }
    }

    pub fn advance(&self) {
        fs::write(self.source.join("file.txt"), "remote update\n").unwrap();
        git(&self.source, &["add", "file.txt"]);
        git(&self.source, &["commit", "-m", "update"]);
        git(&self.source, &["push"]);
    }

    pub fn process(&self, main: bool) -> RepoResult {
        process(&self.repo, main, Duration::from_secs(10))
    }
}

pub fn process(path: &Path, main: bool, timeout: Duration) -> RepoResult {
    let mut runner = GitRunner::new(path, timeout, Cancellation::default());
    repo::process(path.to_path_buf(), main, &mut runner)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot {
    head: Vec<u8>,
    index: Option<Vec<u8>>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}

pub fn snapshot(path: &Path) -> Snapshot {
    fn files(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == ".git" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                files(root, &entry.path(), out);
            } else {
                out.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut tree = BTreeMap::new();
    files(path, path, &mut tree);
    let mut head = fs::read(path.join(".git/HEAD")).unwrap();
    head.extend(command(path, &["rev-parse", "--verify", "HEAD"]).stdout);
    Snapshot {
        head,
        index: fs::read(path.join(".git/index")).ok(),
        files: tree,
    }
}

pub fn executable(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

pub fn cli(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_heads"));
    cmd.arg(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C");
    cmd
}
