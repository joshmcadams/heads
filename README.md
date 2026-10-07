# heads

[![CI](https://github.com/joshmcadams/heads/actions/workflows/ci.yml/badge.svg)](https://github.com/joshmcadams/heads/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Fast-forward every clean Git repository under a folder. Built in Rust, with no
crate dependencies. Uses your system Git and works on Linux and macOS.

## Install

Requires Rust 1.74 or newer, Cargo, Git 2.23 or newer, and Make for the Makefile
commands. Windows is not supported. Install from a local checkout:

```sh
git clone https://github.com/joshmcadams/heads.git
cd heads
make install
heads --version
```

This builds a release binary and installs it to `~/.local/bin` on Linux or
`~/bin` on macOS. Add that directory to your PATH. Remove it with `make uninstall`
from the checkout. See the Make commands below for a custom installation path.

You can also install directly with Cargo:

```sh
cargo install --git https://github.com/joshmcadams/heads.git --locked
heads --version
```

Cargo installs to `~/.cargo/bin`; add that directory to your PATH. This command
builds from the repository's current default branch. The package is not yet
published to crates.io.

For a Cargo installation, remove the binary with `cargo uninstall heads`.

## Usage

```sh
heads [--main] [--parallel N] [--timeout SECONDS] [folder]
heads ~/Development
heads --main --parallel 8 ~/Development
```

The folder defaults to the current directory and is resolved to an absolute path.
The default is five concurrent repositories and a 120-second timeout per
repository. `--parallel` and `--timeout` accept positive integers. Use `--help`
for usage, or `--` before a folder whose name begins with `-`.
Use `--version` (or `-V`) to print the installed version.

Discovery includes hidden directories and `.git` files used by worktrees and
submodules. It does not follow directory symlinks. When a repository is found,
its entire subtree is excluded, including when the scan root itself is a repo.
Unreadable directories are skipped and counted on stderr.

`--main` switches each clean repository to the branch named by
`refs/remotes/origin/HEAD`, falling back to a local `main`, then `master`.
A missing local default branch is created to track `origin/<default>`.
The repository stays on that branch after the command, including if a subsequent
pull fails or the branch has no upstream. Without `--main`, the checked-out branch
is used.

## Output and statuses

The table and summary go to stdout, in lexical repository-path order, regardless
of completion order. Paths are relative to the scan root; the root itself is `.`.
Scanning messages and unreadable-directory counts go to stderr. A progress bar
is shown only when stderr is a terminal, with redraws serialized by one reporter.

```text
REPOSITORY          BRANCH  STATUS
------------------  ------  ---------------
api-service         main    synced
experimental/thing  feat    dirty (skipped)
1 synced · 1 skipped
```

| Status | Meaning |
| --- | --- |
| `synced` | HEAD moved after a successful fast-forward pull. |
| `up-to-date` | Pull succeeded and HEAD stayed the same. |
| `dirty (skipped)` | Tracked changes or untracked files; no switch or pull attempted. |
| `detached (skipped)` | HEAD is detached; no switch or pull attempted. |
| `no upstream (skipped)` | The selected branch has no upstream; no pull attempted. |
| `error: <message>` | A Git command failed, the default branch could not be resolved, or the repository timed out. Git failures include the command and its output, collapsed to one line. |
| `cancelled` | Ctrl-C stopped active work, or the repository never started. |

An unborn HEAD is an error with Git's diagnostic. A detached repository displays
`-` in the branch column. An empty scan prints `No git repositories found.`.
The summary lists nonzero counts for synced, current, skipped, errors and cancelled.

| Exit code | Meaning |
| --- | --- |
| 0 | No repository errored, including scans containing only skips. |
| 1 | At least one repository errored, or the command encountered an I/O failure. |
| 2 | Invalid arguments or scan folder. |
| 130 | Interrupted with Ctrl-C; results are still printed. |

## Safety

- Check the branch and cleanliness before switching or pulling. Untracked files
  count as dirty even when Git's status configuration hides them. Optional Git
  locking is disabled to prevent a status check from refreshing the index.
- Use `git pull --ff-only --no-rebase`: divergence errors rather than creating a
  merge or rebase, even with a configured rebase pull policy. No stash or reset.
- Detect updates by comparing commit IDs, independent of Git's language.
- Close stdin for every Git child and set `GIT_TERMINAL_PROMPT=0`. Set
  `GIT_SSH_COMMAND="ssh -o BatchMode=yes"` unless you already supplied one.
- Apply a deadline to the whole repository procedure. Timeout and Ctrl-C kill
  the Git process group, including its ordinary helper and SSH descendants.
- Honor your Git configuration, credential helpers, SSH agent and hooks by
  invoking the system executable. User hooks/helpers can have their own side
  effects; a custom SSH command remains responsible for its noninteractive options.

Dirty and detached repositories are skipped before any network operation.
Failed pulls can update Git's fetch metadata. `--main` is an intentional branch
change and is not rolled back after a later skip or error. A timeout/interrupt
terminates an operation without rolling back updates already completed by Git.
Avoid changing repositories concurrently while `heads` is running.

## Make commands

Requires **Rust 1.74 or newer** (edition 2021), Cargo, and system Git supporting
`git switch` (Git 2.23+), plus Make. Tests use local bare remotes and executable
fixtures and require Git 2.28+ for `git init --initial-branch`; they require no
network or downloaded crates. Install lint components
with `rustup component add rustfmt clippy`.

```sh
make build       # release binary at ./bin/heads (also the default for make)
make test        # cargo test --offline --locked
make lint        # rustfmt check and Clippy
make package     # verify the source package in target/package
make install     # build and install to ~/bin on macOS, ~/.local/bin on Linux
make uninstall   # remove the installed heads binary
```

Override `INSTALL_DIR` for both installation and removal. Paths containing
spaces are supported:

```sh
make install INSTALL_DIR="$HOME/tools/bin"
make uninstall INSTALL_DIR="$HOME/tools/bin"
```

Use the same directory for both commands. Installation sets executable
permissions and replaces an existing `heads` binary at that path. Uninstall
removes only that binary, keeps the directory and its other contents, and
succeeds if the binary is already absent. Uninstall does not build the project
or require Rust. An empty `INSTALL_DIR` is rejected.

The build and install targets ad hoc sign the binary on macOS with
`codesign --force -s -`; this uses the macOS command-line tools. Add the
installation directory to your PATH. Build, test, lint, and package use Cargo
offline with the committed lockfile.

`make build` produces a statically linked Linux executable by enabling static
CRT linking. macOS builds use native platform linking. A plain `cargo build`
uses the Rust target's default linking; to use musl on Linux, install a matching
Rust target and linker, then build with that target, for example:

```sh
cargo build --offline --locked --release --target x86_64-unknown-linux-musl
```

CI tests stable Rust on Linux and macOS and Rust 1.74 on Linux. Stable jobs also
run lint, build release binaries, check install/uninstall in a temporary directory,
verify Cargo packaging, and upload tarballs
containing the binary, README, and license as workflow artifacts. These are
CI artifacts; they are not tagged releases.

## Contributing and license

Bug reports and focused pull requests are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md) for development and test instructions,
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the implementation, and
[docs/RELEASING.md](docs/RELEASING.md) for maintainer instructions.
See [SECURITY.md](SECURITY.md) to report a vulnerability privately.

Copyright (c) 2026 Josh McAdams. Licensed under the [MIT License](LICENSE).
