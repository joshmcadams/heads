mod support;

use heads::scan::{discover, discover_until};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use support::Temp;

fn repo(path: &Path) {
    fs::create_dir_all(path.join(".git")).unwrap();
}

#[test]
fn root_repo_stops_recursion_even_with_nested_repo() {
    let temp = Temp::new();
    repo(&temp.0);
    repo(&temp.0.join("nested"));
    repo(&temp.0.join(".git/looks-like-a-repo"));
    let found = discover(&temp.0);
    assert_eq!(found.repos, std::slice::from_ref(&temp.0));
    assert_eq!(found.unreadable, 0);
}

#[test]
fn siblings_are_sorted_and_nested_repos_and_symlinks_are_pruned() {
    let temp = Temp::new();
    for name in ["z", "a", "a/nested", ".hidden/cache"] {
        repo(&temp.0.join(name));
    }
    fs::create_dir_all(temp.0.join("worktree")).unwrap();
    fs::write(temp.0.join("worktree/.git"), "gitdir: /elsewhere\n").unwrap();
    symlink(&temp.0, temp.0.join("loop")).unwrap();
    symlink(temp.0.join("z"), temp.0.join("alias")).unwrap();
    let found = discover(&temp.0);
    assert_eq!(
        found.repos,
        [".hidden/cache", "a", "worktree", "z"].map(|name| temp.0.join(name))
    );
    assert_eq!(found.unreadable, 0);
}

#[test]
fn empty_tree_and_git_file_at_root() {
    let temp = Temp::new();
    assert!(discover(&temp.0).repos.is_empty());
    fs::write(temp.0.join(".git"), "gitdir: /elsewhere\n").unwrap();
    repo(&temp.0.join("nested"));
    assert_eq!(discover(&temp.0).repos, std::slice::from_ref(&temp.0));
}

#[test]
fn does_not_follow_git_symlink() {
    let temp = Temp::new();
    fs::create_dir(temp.0.join("real-git")).unwrap();
    symlink(temp.0.join("real-git"), temp.0.join(".git")).unwrap();
    assert!(discover(&temp.0).repos.is_empty());
}

#[test]
fn discovery_can_be_cancelled_before_walking() {
    let temp = Temp::new();
    repo(&temp.0.join("repo"));
    assert!(discover_until(&temp.0, || true).repos.is_empty());
}

#[test]
fn unreadable_directory_is_counted() {
    let temp = Temp::new();
    let blocked = temp.0.join("blocked");
    fs::create_dir(&blocked).unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o0)).unwrap();
    let readable_as_root = fs::read_dir(&blocked).is_ok();
    let found = discover(&temp.0);
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).unwrap();
    if readable_as_root {
        // Root bypasses permission bits. Still exercise the read_dir error path.
        assert_eq!(discover(&temp.0.join("missing")).unreadable, 1);
    } else {
        assert_eq!(found.unreadable, 1);
    }
    assert!(found.repos.is_empty());
}
