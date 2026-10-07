use heads::repo::{process, Output, RunError, Runner, Status};
use std::collections::VecDeque;
use std::path::PathBuf;

struct Script(VecDeque<(Vec<&'static str>, Output)>);

impl Script {
    fn new(steps: Vec<(Vec<&'static str>, i32, &'static str, &'static str)>) -> Self {
        Self(
            steps
                .into_iter()
                .map(|(args, code, stdout, stderr)| {
                    (
                        args,
                        Output {
                            code,
                            stdout: stdout.into(),
                            stderr: stderr.into(),
                        },
                    )
                })
                .collect(),
        )
    }
}

impl Runner for Script {
    fn run(&mut self, args: &[&str]) -> Result<Output, RunError> {
        let (expected, output) = self.0.pop_front().expect("unexpected Git command");
        assert_eq!(args, expected);
        Ok(output)
    }
}

#[test]
fn dirty_stops_before_switch_or_any_network_command() {
    let mut runner = Script::new(vec![
        (
            vec!["symbolic-ref", "--quiet", "--short", "HEAD"],
            0,
            "feature\n",
            "",
        ),
        (vec!["rev-parse", "--verify", "HEAD"], 0, "123\n", ""),
        (
            vec!["status", "--porcelain", "--untracked-files=all"],
            0,
            "?? new\n",
            "",
        ),
    ]);
    let result = process(PathBuf::from("repo"), true, &mut runner);
    assert_eq!(result.status, Status::Dirty);
    assert_eq!(result.branch, "feature");
    assert!(runner.0.is_empty());
}

#[test]
fn detached_stops_after_one_command() {
    let mut runner = Script::new(vec![(
        vec!["symbolic-ref", "--quiet", "--short", "HEAD"],
        1,
        "",
        "",
    )]);
    assert_eq!(
        process(PathBuf::from("repo"), true, &mut runner).status,
        Status::Detached
    );
    assert!(runner.0.is_empty());
}

#[test]
fn sha_comparison_does_not_parse_git_pull_output() {
    for (after, expected) in [("before\n", Status::UpToDate), ("after\n", Status::Synced)] {
        let mut runner = Script::new(vec![
            (
                vec!["symbolic-ref", "--quiet", "--short", "HEAD"],
                0,
                "main\n",
                "",
            ),
            (vec!["rev-parse", "--verify", "HEAD"], 0, "before\n", ""),
            (
                vec!["status", "--porcelain", "--untracked-files=all"],
                0,
                "",
                "",
            ),
            (
                vec!["for-each-ref", "--format=%(upstream)", "refs/heads/main"],
                0,
                "refs/remotes/origin/main\n",
                "",
            ),
            (vec!["rev-parse", "--verify", "HEAD"], 0, "before\n", ""),
            (
                vec!["pull", "--ff-only", "--no-rebase"],
                0,
                "Already up to date.\nGanz anders!\n",
                "",
            ),
            (vec!["rev-parse", "--verify", "HEAD"], 0, after, ""),
        ]);
        assert_eq!(
            process(PathBuf::from("repo"), false, &mut runner).status,
            expected
        );
        assert!(runner.0.is_empty());
    }
}

#[test]
fn errors_include_command_and_both_output_streams_collapsed() {
    let mut runner = Script::new(vec![(
        vec!["symbolic-ref", "--quiet", "--short", "HEAD"],
        128,
        "output\n",
        "fatal:\n  broken\t repo\n",
    )]);
    let result = process(PathBuf::from("repo"), false, &mut runner);
    assert_eq!(
        result.status.to_string(),
        "error: git symbolic-ref --quiet --short HEAD (exit 128): output fatal: broken repo"
    );
    assert!(runner.0.is_empty());
}
