pub mod process;
pub mod repo;
pub mod report;
pub mod scan;

use process::{Cancellation, GitRunner};
use repo::{RepoResult, Status};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

pub fn sync_repos(
    paths: &[PathBuf],
    main: bool,
    parallel: usize,
    timeout: Duration,
    cancellation: Cancellation,
    mut completed: impl FnMut(),
) -> Vec<RepoResult> {
    assert!(parallel > 0);
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    let mut results: Vec<_> = paths
        .iter()
        .map(|path| RepoResult {
            path: path.clone(),
            branch: "-".into(),
            status: Status::Cancelled,
        })
        .collect();
    std::thread::scope(|scope| {
        for _ in 0..parallel.min(paths.len()) {
            let sender = sender.clone();
            let cancellation = cancellation.clone();
            let next = &next;
            scope.spawn(move || loop {
                if cancellation.is_cancelled() {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = paths.get(index) else { break };
                let mut runner = GitRunner::new(path, timeout, cancellation.clone());
                let result = repo::process(path.clone(), main, &mut runner);
                if sender.send((index, result)).is_err() {
                    break;
                }
            });
        }
        drop(sender);
        for (index, result) in receiver {
            results[index] = result;
            completed();
        }
    });
    results
}
