# Working on heads

heads is a dependency-free Rust CLI that discovers Git repositories and
fast-forwards clean tracking branches. Read README.md for the user-facing
contract and docs/ARCHITECTURE.md for the module boundaries.

## Development

- Support Linux and macOS. Keep Rust 1.74 compatibility and the Cargo.lock file.
  Unix process groups and signals are intentional; Windows is unsupported.
- Runtime Git must support `git switch` (2.23+); tests use
  `git init --initial-branch` and require Git 2.28+.
- Prefer the standard library. Discuss a new crate dependency in the change
  description rather than adding one incidentally.
- Use `make test` for offline tests, `make lint` for rustfmt and Clippy,
  `make build` for `bin/heads`, and `make package` for source-package validation.
  Run the checks appropriate to the change before publishing it.
- For installation changes, run `make install INSTALL_DIR="<temporary path>"`,
  execute the installed binary with `--version`, and run `make uninstall` with
  the same directory twice. Verify that unrelated files remain. Include a path
  containing spaces. CI also checks macOS signing.
- Keep Make commands and installation instructions consistent across README.md,
  CONTRIBUTING.md, and CI. Uninstall must not build or delete the directory.

## Code and tests

- `src/main.rs` handles arguments and exit codes; `scan.rs` handles discovery;
  `repo.rs` defines the ordered procedure through `Runner`; `process.rs` runs
  Git and handles deadlines/cancellation; `report.rs` formats results; `lib.rs`
  bounds concurrency and preserves discovery order.
- Preserve fast-forward-only pulls, explicit untracked-file checks, disabled
  optional Git locking, and skips before switching or fetching dirty/detached
  repositories. Never add automatic stash, reset, merge, or rebase behavior.
- `--main` deliberately retains a successful branch switch after a later skip
  or error. Compare commit IDs to detect updates; do not parse translated Git
  messages. Keep these behaviors documented and tested.
- A deadline covers the whole repository procedure and output capture. Cleanup
  must terminate Git's process group, including helpers, and reap the child.
  Keep stdin closed and Git noninteractive, while honoring a supplied SSH command.
- Tests belong in `tests/`, with isolated Git fixtures in `tests/support/mod.rs`.
  Use local bare remotes and executable helpers; tests must not require network
  access or operate on a developer's existing repositories.
- Canonicalize paths when asserting CLI scan-root output: macOS temporary paths
  can traverse symlinks. Deadline tests must verify the timeout contract without
  assuming which command reaches a deadline on a busy runner.
- Smoke-test with `--help`, `--version`, or temporary repositories. Do not run
  the syncing CLI over unrelated working directories as a routine check.

## Documentation and commits

- Update README.md for changes to CLI options, statuses, exit codes, safety,
  platform requirements, or installation. Keep source packages usable with Make.
- Use `61422+joshmcadams@users.noreply.github.com` for both author and committer
  email addresses on maintainer commits. Do not change global Git configuration
  or include personal email addresses, tokens, or local credentials.
- Never commit `target/`, `bin/`, environment files, or local agent/credential
  directories. The project and contributions are licensed under MIT.
