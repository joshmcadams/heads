# Architecture

heads is a Rust CLI with no crate dependencies. It invokes system Git and uses
Unix process groups for cancellation, so the supported platforms are Linux and
macOS. The public CLI behavior is documented in [README.md](../README.md).

## Modules

| Module | Responsibility |
| --- | --- |
| `main.rs` | Arguments, signal handler installation, discovery, progress, results, exit code. |
| `scan.rs` | Sorted discovery, hidden directories, `.git` files, subtree pruning, unreadable counts. |
| `repo.rs` | Ordered repository procedure behind the `Runner` trait and typed statuses. |
| `process.rs` | System Git, closed stdin, noninteractive defaults, deadline, cancellation, output capture, process groups. |
| `report.rs` | Aligned table, summary, and serialized progress on terminal stderr. |
| `lib.rs` | Bounded workers and results indexed by discovery order. |

## Repository procedure

1. Resolve the current branch. Skip detached HEAD; report unborn HEAD as an error.
2. Inspect tracked and untracked changes with optional Git locking disabled.
   Skip dirty repositories before any branch change or network operation.
3. With `--main`, resolve `origin/HEAD`, then local `main`, then local `master`.
   Switch to the selected branch, creating explicit tracking of `origin` if needed.
4. Skip a selected branch with no upstream.
5. Run `git pull --ff-only --no-rebase` and compare HEAD commit IDs to distinguish
   updates from already-current repositories, independently of Git's output language.

A successful branch switch remains in effect if a later pull fails or the branch
has no upstream. Failed pulls can update fetch metadata. Hooks and helpers retain
their configured behavior. These limits are part of the documented contract.

## Cancellation and ordering

Every repository has one deadline covering all Git commands and output capture.
Each Git child starts its own process group. Timeout, interruption, and child
cleanup kill that group and reap the child. Reader threads drain stdout and stderr
concurrently to avoid pipe deadlock; pipe completion remains bounded by the deadline.

The signal handler only stores an atomic cancellation flag. Workers stop claiming
new repositories when cancelled. Results start as `cancelled` and are replaced on
completion; the table preserves lexical discovery order even when workers finish
out of order. The main thread owns progress reporting.

## Validation

Offline integration tests use local bare remotes and temporary clones to verify
branch behavior, dirty/index preservation, divergence, worktrees, unreachable
remotes, deadlines, interruption, helper descendants, and worker limits. Scripted
runners verify command ordering; golden output tests cover reporting and progress.
CI also checks formatting, Clippy, package contents, and release builds.
