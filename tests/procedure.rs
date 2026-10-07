mod support;

use heads::repo::Status;
use std::fs;
use std::time::{Duration, Instant};
use support::{git, process, snapshot, Fixture, Temp};

#[test]
fn clean_behind_syncs_and_second_run_is_up_to_date() {
    let fixture = Fixture::new();
    fixture.advance();
    assert_eq!(fixture.process(false).status, Status::Synced);
    assert_eq!(
        git(&fixture.repo, &["rev-parse", "HEAD"]),
        git(&fixture.source, &["rev-parse", "HEAD"])
    );
    let before = snapshot(&fixture.repo);
    assert_eq!(fixture.process(false).status, Status::UpToDate);
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn dirty_untracked_modified_and_staged_repos_remain_byte_identical_even_with_main() {
    for dirty in ["untracked", "modified", "staged"] {
        let fixture = Fixture::new();
        git(&fixture.repo, &["switch", "-c", "feature"]);
        fixture.advance();
        // Explicit untracked checking must override the user's status setting.
        git(
            &fixture.repo,
            &["config", "status.showUntrackedFiles", "no"],
        );
        if dirty == "untracked" {
            fs::write(fixture.repo.join("new.txt"), "untracked\n").unwrap();
        } else {
            fs::write(fixture.repo.join("file.txt"), "local change\n").unwrap();
            if dirty == "staged" {
                git(&fixture.repo, &["add", "file.txt"]);
            }
        }
        let before = snapshot(&fixture.repo);
        let refs = git(&fixture.repo, &["show-ref"]);
        let fetch_head = fs::read(fixture.repo.join(".git/FETCH_HEAD")).ok();
        assert_eq!(fixture.process(true).status, Status::Dirty);
        assert_eq!(snapshot(&fixture.repo), before);
        assert_eq!(git(&fixture.repo, &["show-ref"]), refs);
        assert_eq!(
            fs::read(fixture.repo.join(".git/FETCH_HEAD")).ok(),
            fetch_head
        );
    }
}

#[test]
fn detached_and_no_upstream_are_unchanged_skips() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["switch", "--detach"]);
    let before = snapshot(&fixture.repo);
    assert_eq!(fixture.process(true).status, Status::Detached);
    assert_eq!(snapshot(&fixture.repo), before);
    git(&fixture.repo, &["switch", "-c", "local"]);
    let before = snapshot(&fixture.repo);
    assert_eq!(fixture.process(false).status, Status::NoUpstream);
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn unborn_head_errors_with_git_stderr_without_touching_tree_or_index() {
    let temp = Temp::new();
    git(&temp.0, &["init", "--initial-branch=main"]);
    fs::write(temp.0.join("new.txt"), "untracked").unwrap();
    let before = snapshot(&temp.0);
    let result = process(&temp.0, false, Duration::from_secs(10));
    let Status::Error(message) = result.status else {
        panic!("expected error")
    };
    assert!(message.contains("git rev-parse --verify HEAD"), "{message}");
    assert!(message.contains("fatal:"), "{message}");
    assert_eq!(snapshot(&temp.0), before);
}

#[test]
fn main_switches_feature_and_syncs() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["switch", "-c", "feature"]);
    fixture.advance();
    let result = fixture.process(true);
    assert_eq!(result.branch, "main");
    assert_eq!(result.status, Status::Synced);
    assert_eq!(git(&fixture.repo, &["branch", "--show-current"]), "main");
}

#[test]
fn main_falls_back_to_local_main_then_master() {
    for branch in ["main", "master"] {
        let fixture = Fixture::new();
        git(
            &fixture.repo,
            &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
        );
        if branch == "master" {
            git(&fixture.repo, &["branch", "-m", "main", "master"]);
        }
        git(&fixture.repo, &["switch", "-c", "feature"]);
        fixture.advance();
        let result = fixture.process(true);
        assert_eq!(result.branch, branch);
        assert_eq!(result.status, Status::Synced);
    }
}

#[test]
fn main_creates_missing_local_default_tracking_origin_even_with_another_remote() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["switch", "-c", "feature"]);
    git(&fixture.repo, &["branch", "-D", "main"]);
    git(
        &fixture.repo,
        &["remote", "add", "other", fixture.remote.to_str().unwrap()],
    );
    git(&fixture.repo, &["fetch", "other"]);
    fixture.advance();
    let result = fixture.process(true);
    assert_eq!(result.branch, "main");
    assert_eq!(result.status, Status::Synced);
    assert_eq!(
        git(&fixture.repo, &["rev-parse", "--abbrev-ref", "@{upstream}"]),
        "origin/main"
    );
}

#[test]
fn diverged_branch_errors_without_merging_or_rebasing_even_with_rebase_config() {
    for rebase in ["false", "true"] {
        let fixture = Fixture::new();
        git(&fixture.repo, &["config", "pull.rebase", rebase]);
        fixture.advance();
        fs::write(fixture.repo.join("local.txt"), "local commit\n").unwrap();
        git(&fixture.repo, &["add", "local.txt"]);
        git(&fixture.repo, &["commit", "-m", "local"]);
        let before = snapshot(&fixture.repo);
        let result = fixture.process(false);
        assert!(
            matches!(result.status, Status::Error(_)),
            "{:?}",
            result.status
        );
        assert_eq!(snapshot(&fixture.repo), before);
        assert!(!fixture.repo.join(".git/rebase-merge").exists());
        assert!(!fixture.repo.join(".git/MERGE_HEAD").exists());
    }
}

#[test]
fn unreachable_remote_errors_promptly_and_preserves_repo() {
    let fixture = Fixture::new();
    git(
        &fixture.repo,
        &["remote", "set-url", "origin", "https://127.0.0.1:1/x"],
    );
    let before = snapshot(&fixture.repo);
    let start = Instant::now();
    let result = process(&fixture.repo, false, Duration::from_secs(2));
    assert!(matches!(result.status, Status::Error(_)));
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn failed_main_resolution_and_failed_switch_preserve_repo() {
    let fixture = Fixture::new();
    git(
        &fixture.repo,
        &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
    );
    git(&fixture.repo, &["branch", "-m", "main", "custom"]);
    let before = snapshot(&fixture.repo);
    assert!(matches!(fixture.process(true).status, Status::Error(_)));
    assert_eq!(snapshot(&fixture.repo), before);
    git(
        &fixture.repo,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/missing",
        ],
    );
    assert!(matches!(fixture.process(true).status, Status::Error(_)));
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn main_switch_is_retained_when_default_has_no_upstream() {
    let fixture = Fixture::new();
    git(&fixture.repo, &["branch", "--unset-upstream"]);
    git(&fixture.repo, &["switch", "-c", "feature"]);
    let result = fixture.process(true);
    assert_eq!(result.status, Status::NoUpstream);
    assert_eq!(result.branch, "main");
    assert_eq!(git(&fixture.repo, &["branch", "--show-current"]), "main");
}

#[test]
fn a_real_worktree_with_a_git_file_can_sync() {
    let fixture = Fixture::new();
    let worktree = fixture.temp.0.join("worktree");
    git(
        &fixture.repo,
        &[
            "worktree",
            "add",
            "--track",
            "-b",
            "feature",
            worktree.to_str().unwrap(),
            "origin/main",
        ],
    );
    assert!(worktree.join(".git").is_file());
    assert_eq!(
        heads::scan::discover(&worktree).repos,
        std::slice::from_ref(&worktree)
    );
    fixture.advance();
    assert_eq!(
        process(&worktree, false, Duration::from_secs(10)).status,
        Status::Synced
    );
    assert_eq!(
        process(&worktree, false, Duration::from_secs(10)).status,
        Status::UpToDate
    );
}
