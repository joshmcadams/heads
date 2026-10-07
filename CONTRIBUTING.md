# Contributing

Open an issue or a focused pull request at https://github.com/joshmcadams/heads.
For security reports, follow [SECURITY.md](SECURITY.md).

Use Linux or macOS, Git 2.23+, and Rust 1.74+. Install rustfmt and Clippy with
`rustup component add rustfmt clippy`. From a checkout:

```sh
make test
make lint
make build
make package
```

Tests use temporary repositories, local bare remotes, and shell fixtures. They
do not access your repositories or require network access. Keep new tests local
and independent of global Git configuration. CI runs stable Rust on both
supported platforms and the minimum Rust version on Linux.

Maintain the fast-forward-only behavior. Cover changes to branch selection,
cleanliness, deadlines, or cancellation with tests that verify repository state
and process cleanup. Document user-visible behavior and exit codes in README.md.
The project currently has no crate dependencies; discuss additions in the pull
request and preserve the declared minimum Rust version.

See [the architecture](docs/ARCHITECTURE.md) for module boundaries. Contributions
are made under this project's [MIT license](LICENSE).
