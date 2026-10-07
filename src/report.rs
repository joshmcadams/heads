use crate::repo::{RepoResult, Status};
use std::io::{self, Write};
use std::path::Path;

pub fn table(root: &Path, results: &[RepoResult]) -> String {
    if results.is_empty() {
        return "No git repositories found.\n".to_string();
    }
    let rows: Vec<_> = results
        .iter()
        .map(|result| {
            let relative = result.path.strip_prefix(root).unwrap_or(&result.path);
            let name = if relative.as_os_str().is_empty() {
                ".".to_string()
            } else {
                relative.to_string_lossy().into_owned()
            };
            // Keep paths/branches from injecting additional table rows.
            (
                name.escape_debug().to_string(),
                result.branch.escape_debug().to_string(),
                result.status.to_string(),
            )
        })
        .collect();
    let widths = rows.iter().fold([10, 6, 6], |widths, row| {
        [
            widths[0].max(row.0.chars().count()),
            widths[1].max(row.1.chars().count()),
            widths[2].max(row.2.chars().count()),
        ]
    });
    let mut output = String::new();
    let mut row = |repo: &str, branch: &str, status: &str| {
        output.push_str(repo);
        output.push_str(&" ".repeat(widths[0] - repo.chars().count() + 2));
        output.push_str(branch);
        output.push_str(&" ".repeat(widths[1] - branch.chars().count() + 2));
        output.push_str(status);
        output.push('\n');
    };
    row("REPOSITORY", "BRANCH", "STATUS");
    row(
        &"-".repeat(widths[0]),
        &"-".repeat(widths[1]),
        &"-".repeat(widths[2]),
    );
    for (name, branch, status) in rows {
        row(&name, &branch, &status);
    }
    output.push_str(&summary(results));
    output.push('\n');
    output
}

pub fn summary(results: &[RepoResult]) -> String {
    let mut counts = [0usize; 5];
    for result in results {
        counts[match result.status {
            Status::Synced => 0,
            Status::UpToDate => 1,
            Status::Dirty | Status::Detached | Status::NoUpstream => 2,
            Status::Error(_) => 3,
            Status::Cancelled => 4,
        }] += 1;
    }
    let mut parts = Vec::new();
    for (count, label) in
        counts
            .into_iter()
            .zip(["synced", "up-to-date", "skipped", "error", "cancelled"])
    {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    parts.join(" · ")
}

pub struct Progress<W: Write> {
    writer: W,
    tty: bool,
    total: usize,
    completed: usize,
}

impl<W: Write> Progress<W> {
    pub fn new(writer: W, tty: bool, total: usize) -> Self {
        Self {
            writer,
            tty,
            total,
            completed: 0,
        }
    }

    pub fn advance(&mut self) -> io::Result<()> {
        self.completed += 1;
        if self.tty && self.total > 0 {
            let filled = self.completed * 20 / self.total;
            write!(
                self.writer,
                "\r[{}{}] {}/{}",
                "=".repeat(filled),
                " ".repeat(20 - filled),
                self.completed,
                self.total
            )?;
            self.writer.flush()?;
        }
        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if self.tty && self.total > 0 {
            writeln!(self.writer)?;
        }
        Ok(())
    }
}
