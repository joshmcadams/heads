use heads::repo::{RepoResult, Status};
use heads::report::{self, Progress};
use std::path::{Path, PathBuf};

fn row(path: &str, branch: &str, status: Status) -> RepoResult {
    RepoResult {
        path: PathBuf::from(path),
        branch: branch.into(),
        status,
    }
}

#[test]
fn table_and_summary_golden() {
    let results = [
        row("/root", "main", Status::UpToDate),
        row("/root/api-service", "main", Status::Synced),
        row("/root/experimental/thing", "feat", Status::Dirty),
        row(
            "/root/failed",
            "main",
            Status::Error("fatal:\n  failed\t now".into()),
        ),
    ];
    assert_eq!(
        report::table(Path::new("/root"), &results),
        include_str!("golden/table.txt")
    );
    assert_eq!(
        report::summary(&results),
        "1 synced · 1 up-to-date · 1 skipped · 1 error"
    );
}

#[test]
fn every_status_and_cancellation_summary() {
    let results = [
        Status::Dirty,
        Status::Detached,
        Status::NoUpstream,
        Status::Cancelled,
    ]
    .into_iter()
    .map(|status| row("/root", "-", status))
    .collect::<Vec<_>>();
    assert_eq!(report::summary(&results), "3 skipped · 1 cancelled");
    assert_eq!(Status::Detached.to_string(), "detached (skipped)");
    assert_eq!(Status::NoUpstream.to_string(), "no upstream (skipped)");
    assert_eq!(Status::Cancelled.to_string(), "cancelled");
}

#[test]
fn empty_table_golden() {
    assert_eq!(
        report::table(Path::new("/root"), &[]),
        "No git repositories found.\n"
    );
}

#[test]
fn non_tty_progress_writes_nothing() {
    let mut output = Vec::new();
    let mut progress = Progress::new(&mut output, false, 2);
    progress.advance().unwrap();
    progress.advance().unwrap();
    progress.finish().unwrap();
    assert_eq!(output, b"");
}

#[test]
fn tty_progress_golden() {
    let mut output = Vec::new();
    let mut progress = Progress::new(&mut output, true, 2);
    progress.advance().unwrap();
    progress.advance().unwrap();
    progress.finish().unwrap();
    assert_eq!(
        output,
        b"\r[==========          ] 1/2\r[====================] 2/2\n"
    );
}

#[test]
fn paths_cannot_inject_table_rows() {
    let result = row("/root/line\nbreak", "main", Status::UpToDate);
    let output = report::table(Path::new("/root"), &[result]);
    assert!(output.contains("line\\nbreak"));
    assert_eq!(output.lines().count(), 4);
}
