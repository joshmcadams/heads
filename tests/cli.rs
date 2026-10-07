mod support;

use heads::process::Cancellation;
use heads::repo::Status;
use heads::sync_repos;
use std::fs;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};
use support::{cli, executable, git, snapshot, Fixture, Temp};

extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn wait(mut child: Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            unsafe {
                kill(-(child.id() as i32), 9);
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!("CLI did not finish within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn spawn(cmd: &mut Command) -> Child {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap()
}

fn wait_for_file(path: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fs::read_to_string(path)
        .map(|pids| pids.split_whitespace().count() == 2)
        .unwrap_or(false)
    {
        assert!(
            child.try_wait().unwrap().is_none(),
            "CLI exited before helper started"
        );
        assert!(Instant::now() < deadline, "helper never started");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn alive(pid: u32) -> bool {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    output.status.success() && !status.trim().is_empty() && !status.trim_start().starts_with('Z')
}

fn assert_dead(marker: &Path) {
    let pids = fs::read_to_string(marker).unwrap();
    assert!(
        !pids.trim().is_empty(),
        "helper did not record its process IDs"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    for pid in pids.split_whitespace().map(|pid| pid.parse().unwrap()) {
        while alive(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!alive(pid), "helper process {pid} survived cancellation");
    }
}

fn sleepy_remote(fixture: &Fixture) -> (std::path::PathBuf, std::path::PathBuf) {
    let helpers = fixture.temp.0.join("helpers");
    fs::create_dir(&helpers).unwrap();
    let marker = fixture.temp.0.join("pids");
    executable(&helpers.join("git-remote-heads-sleep"), "#!/bin/sh\nsleep 60 &\nchild=$!\nprintf '%s %s\\n' \"$$\" \"$child\" > \"$HEADS_TEST_PIDS\"\nwait \"$child\"\n");
    git(
        &fixture.repo,
        &["remote", "set-url", "origin", "heads-sleep::ignored"],
    );
    git(
        &fixture.repo,
        &["config", "protocol.heads-sleep.allow", "always"],
    );
    (helpers, marker)
}

fn helper_env(cmd: &mut Command, helpers: &Path, marker: &Path) {
    let mut paths = vec![helpers.to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    cmd.env("PATH", std::env::join_paths(paths).unwrap())
        .env("HEADS_TEST_PIDS", marker);
}

#[test]
fn usage_errors_exit_two_and_help_exits_zero() {
    let temp = Temp::new();
    for args in [
        vec!["--parallel", "0"],
        vec!["--parallel", "-1"],
        vec!["--parallel"],
        vec!["--parallel", "bad"],
        vec!["--timeout", "0"],
        vec!["--timeout", "bad"],
        vec!["--timeout", "18446744073709551615"],
        vec!["--unknown"],
        vec!["extra"],
    ] {
        let output = cli(&temp.0).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
    let output = cli(&temp.0).arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(output.stderr.is_empty());
    let output = cli(&temp.0.join("missing")).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    for flag in ["--version", "-V"] {
        let output = cli(&temp.0.join("missing")).arg(flag).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("heads {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn empty_tree_non_tty_output_golden_and_default_folder() {
    let temp = Temp::new();
    let output = Command::new(env!("CARGO_BIN_EXE_heads"))
        .current_dir(&temp.0)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"No git repositories found.\n");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "Scanning {}…\nFound 0 repositories.\n",
            temp.0.canonicalize().unwrap().display()
        )
    );
}

#[test]
fn root_repo_is_reported_as_dot_without_descending_and_second_run_is_current() {
    let fixture = Fixture::new();
    fixture.advance();
    fs::create_dir_all(fixture.repo.join("nested/.git")).unwrap();
    // Ignore this fixture so the outer repo remains clean.
    fs::write(fixture.repo.join(".git/info/exclude"), "nested/\n").unwrap();
    let first = cli(&fixture.repo).output().unwrap();
    assert!(first.status.success());
    assert!(String::from_utf8_lossy(&first.stdout).contains("synced"));
    assert!(String::from_utf8_lossy(&first.stderr).contains("Found 1 repositories."));
    let second = cli(&fixture.repo).output().unwrap();
    assert!(second.status.success());
    assert_eq!(String::from_utf8_lossy(&second.stdout), "REPOSITORY  BRANCH  STATUS\n----------  ------  ----------\n.           main    up-to-date\n1 up-to-date\n");
    assert!(!second.stderr.contains(&b'\r'));
}

#[test]
fn acceptance_mixed_tree_exits_one_preserves_all_four_and_sorts_output() {
    let temp = Temp::new();
    let mut fixtures = Vec::new();
    for name in ["dirty", "detached", "unreachable", "diverged"] {
        let fixture = Fixture::new();
        let path = temp.0.join(name);
        fs::rename(&fixture.repo, &path).unwrap();
        if name == "dirty" {
            fs::write(path.join("new.txt"), "untracked").unwrap();
        }
        if name == "detached" {
            git(&path, &["switch", "--detach"]);
        }
        if name == "unreachable" {
            git(
                &path,
                &["remote", "set-url", "origin", "https://127.0.0.1:1/x"],
            );
        }
        if name == "diverged" {
            fixture.advance();
            fs::write(path.join("local.txt"), "local").unwrap();
            git(&path, &["add", "."]);
            git(&path, &["commit", "-m", "local"]);
        }
        fixtures.push((fixture, path.clone(), snapshot(&path)));
    }
    let output = wait(
        spawn(cli(&temp.0).arg("--timeout").arg("3")),
        Duration::from_secs(8),
    );
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let rows: Vec<_> = stdout.lines().skip(2).take(4).collect();
    assert!(rows[0].starts_with("detached ") && rows[0].ends_with("detached (skipped)"));
    assert!(rows[1].starts_with("dirty ") && rows[1].ends_with("dirty (skipped)"));
    assert!(rows[2].starts_with("diverged ") && rows[2].contains("error:"));
    assert!(rows[3].starts_with("unreachable ") && rows[3].contains("error:"));
    assert!(stdout.ends_with("2 skipped · 2 error\n"));
    for (_, path, before) in fixtures {
        assert_eq!(snapshot(&path), before);
    }
}

#[test]
fn timeout_kills_remote_helper_and_its_descendant_and_preserves_repo() {
    let fixture = Fixture::new();
    let (helpers, marker) = sleepy_remote(&fixture);
    let before = snapshot(&fixture.repo);
    let mut cmd = cli(&fixture.repo);
    cmd.args(["--timeout", "1"]);
    helper_env(&mut cmd, &helpers, &marker);
    let started = Instant::now();
    let output = wait(spawn(&mut cmd), Duration::from_secs(5));
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("error: timed out after 1s (git pull"));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_dead(&marker);
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn ctrl_c_kills_active_children_and_reports_never_started_repos() {
    let fixture = Fixture::new();
    let (helpers, marker) = sleepy_remote(&fixture);
    let root = fixture.temp.0.join("tree");
    fs::create_dir(&root).unwrap();
    let first = root.join("a-running");
    fs::rename(&fixture.repo, &first).unwrap();
    git(
        &fixture.temp.0,
        &[
            "clone",
            fixture.remote.to_str().unwrap(),
            root.join("b-pending").to_str().unwrap(),
        ],
    );
    let before = snapshot(&first);
    let mut cmd = cli(&root);
    cmd.args(["--parallel", "1"]);
    helper_env(&mut cmd, &helpers, &marker);
    let mut child = spawn(&mut cmd);
    wait_for_file(&marker, &mut child);
    unsafe {
        assert_eq!(kill(child.id() as i32, 2), 0);
    }
    let output = wait(child, Duration::from_secs(5));
    assert_eq!(output.status.code(), Some(130));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a-running") && stdout.contains("b-pending"));
    assert!(stdout.ends_with("2 cancelled\n"));
    assert_dead(&marker);
    assert_eq!(snapshot(&first), before);
}

#[test]
fn every_git_child_has_closed_stdin_and_noninteractive_env_and_preserves_ssh_override() {
    for ssh in [None, Some("custom-ssh --option")] {
        let fixture = Fixture::new();
        let helpers = fixture.temp.0.join("helpers");
        fs::create_dir(&helpers).unwrap();
        let git_path = Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        let git_path = String::from_utf8(git_path.stdout).unwrap();
        let log = fixture.temp.0.join("env-log");
        executable(&helpers.join("git"), &format!("#!/bin/sh\n[ \"$GIT_TERMINAL_PROMPT\" = 0 ] || exit 97\nif read -r line; then exit 98; fi\nprintf '%s\\n' \"$GIT_SSH_COMMAND\" >> \"$HEADS_TEST_LOG\"\nexec '{}' \"$@\"\n", git_path.trim()));
        let mut cmd = cli(&fixture.repo);
        helper_env(&mut cmd, &helpers, &log);
        cmd.env("HEADS_TEST_LOG", &log)
            .env_remove("GIT_SSH_COMMAND");
        if let Some(ssh) = ssh {
            cmd.env("GIT_SSH_COMMAND", ssh);
        }
        let output = wait(spawn(&mut cmd), Duration::from_secs(5));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let log = fs::read_to_string(log).unwrap();
        assert!(log.lines().count() >= 7);
        assert!(log
            .lines()
            .all(|line| line == ssh.unwrap_or("ssh -o BatchMode=yes")));
    }
}

#[test]
fn pre_cancelled_worker_pool_returns_all_results_in_discovery_order() {
    let paths = ["/a", "/b", "/c"].map(std::path::PathBuf::from);
    let cancel = Cancellation::default();
    cancel.cancel();
    let results = sync_repos(&paths, false, 2, Duration::from_secs(1), cancel, || {
        panic!("no worker should start")
    });
    assert_eq!(
        results
            .iter()
            .map(|result| &result.path)
            .collect::<Vec<_>>(),
        paths.iter().collect::<Vec<_>>()
    );
    assert!(results
        .iter()
        .all(|result| result.status == Status::Cancelled));
}

#[test]
fn worker_limit_is_enforced_and_table_order_survives_out_of_order_completion() {
    let temp = Temp::new();
    let root = temp.0.join("tree");
    let helpers = temp.0.join("helpers");
    let state = temp.0.join("state");
    fs::create_dir(&helpers).unwrap();
    fs::create_dir(&state).unwrap();
    for name in ["a", "b", "c", "d"] {
        fs::create_dir_all(root.join(name).join(".git")).unwrap();
    }
    executable(
        &helpers.join("git"),
        r#"#!/bin/sh
shift # --no-optional-locks
name=${PWD##*/}
lock() { while ! mkdir "$HEADS_TEST_STATE/lock" 2>/dev/null; do sleep 0.01; done; }
unlock() { rmdir "$HEADS_TEST_STATE/lock"; }
case "$1" in
symbolic-ref)
    lock
    active=$(cat "$HEADS_TEST_STATE/active" 2>/dev/null || printf 0)
    maximum=$(cat "$HEADS_TEST_STATE/max" 2>/dev/null || printf 0)
    active=$((active + 1))
    printf '%s' "$active" > "$HEADS_TEST_STATE/active"
    if [ "$active" -gt "$maximum" ]; then printf '%s' "$active" > "$HEADS_TEST_STATE/max"; fi
    unlock
    if [ "$name" = a ]; then sleep 0.4; else sleep 0.04; fi
    printf 'main\n'
    ;;
rev-parse)
    count=$(cat "$HEADS_TEST_STATE/$name-heads" 2>/dev/null || printf 0)
    count=$((count + 1))
    printf '%s' "$count" > "$HEADS_TEST_STATE/$name-heads"
    if [ "$count" -eq 3 ]; then
        lock
        active=$(cat "$HEADS_TEST_STATE/active")
        printf '%s' "$((active - 1))" > "$HEADS_TEST_STATE/active"
        printf '%s\n' "$name" >> "$HEADS_TEST_STATE/finished"
        unlock
    fi
    printf 'same-sha\n'
    ;;
for-each-ref) printf 'refs/remotes/origin/main\n' ;;
status|pull) ;;
*) exit 99 ;;
esac
"#,
    );
    let mut cmd = cli(&root);
    helper_env(&mut cmd, &helpers, &state);
    cmd.env("HEADS_TEST_STATE", &state)
        .args(["--parallel", "2"]);
    let output = wait(spawn(&mut cmd), Duration::from_secs(5));
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(state.join("max")).unwrap(), "2");
    assert_eq!(fs::read_to_string(state.join("active")).unwrap(), "0");
    let finished = fs::read_to_string(state.join("finished")).unwrap();
    assert!(finished.starts_with("b\n"), "{finished}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let names: Vec<_> = stdout
        .lines()
        .skip(2)
        .take(4)
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(names, ["a", "b", "c", "d"]);
    assert!(stdout.ends_with("4 up-to-date\n"));
}

#[test]
fn timeout_budget_is_shared_by_all_commands_for_one_repository() {
    let temp = Temp::new();
    let root = temp.0.join("repo");
    let helpers = temp.0.join("helpers");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir(&helpers).unwrap();
    executable(&helpers.join("git"), "#!/bin/sh\nshift\nsleep 0.45\ncase \"$1\" in\nsymbolic-ref) printf 'main\\n' ;;\nrev-parse) printf 'sha\\n' ;;\nstatus) ;;\n*) exit 99 ;;\nesac\n");
    let mut cmd = cli(&root);
    helper_env(&mut cmd, &helpers, &temp.0);
    cmd.args(["--timeout", "1"]);
    let started = Instant::now();
    let output = wait(spawn(&mut cmd), Duration::from_secs(4));
    assert_eq!(output.status.code(), Some(1));
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(String::from_utf8_lossy(&output.stdout).contains("timed out after 1s (git status"));
}

#[test]
fn deadline_also_bounds_output_capture_after_git_exits() {
    let temp = Temp::new();
    let root = temp.0.join("repo");
    let helpers = temp.0.join("helpers");
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir(&helpers).unwrap();
    let marker = temp.0.join("pids");
    executable(&helpers.join("git"), "#!/bin/sh\nsleep 60 &\nprintf '%s\\n' \"$!\" > \"$HEADS_TEST_PIDS\"\nprintf 'main\\n'\nexit 0\n");
    let mut cmd = cli(&root);
    helper_env(&mut cmd, &helpers, &marker);
    cmd.args(["--timeout", "1"]);
    let output = wait(spawn(&mut cmd), Duration::from_secs(4));
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("timed out after 1s (git symbolic-ref")
    );
    assert_dead(&marker);
}

#[test]
fn stdin_is_not_consumed_by_cli() {
    let temp = Temp::new();
    let mut child = cli(&temp.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Keep the write end open: the CLI should finish without waiting for EOF.
    let _stdin = child.stdin.take().unwrap();
    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    assert!(child.wait().unwrap().success());
}
