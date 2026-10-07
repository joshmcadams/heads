# Publication and releases

## GitHub settings

The source repository is https://github.com/joshmcadams/heads. From an environment
with GitHub network access and an admin-authenticated GitHub CLI, run:

```sh
bash scripts/configure-github.sh
```

The script first requires all three CI jobs to have passed on the current main
commit. It applies the repository description and topics, enables issues,
disables the unused wiki and projects, selects squash merging, and deletes
merged branches. It sets the default Actions token to read access, disallows
workflow PR approvals, enables vulnerability alerts, secret scanning, push
protection, and private vulnerability reporting.

It replaces classic main branch protection with required pull requests, all
three CI checks from the GitHub Actions app, an up-to-date branch, resolved
conversations, and linear history. Force pushes and branch deletion are blocked.
The policy applies to admins. Zero review approvals are required so a sole
maintainer can merge their own PR after CI passes. Adjust that count if more
maintainers join.

GitHub endpoints are documented in the [repository API](https://docs.github.com/en/rest/repos/repos),
[branch protection API](https://docs.github.com/en/rest/branches/branch-protection),
and [Actions permissions API](https://docs.github.com/en/rest/actions/permissions).
The script stops on a failed request and can be rerun after resolving it.
It requires administration and Actions write permissions; it does not need a
secret embedded in the repository. Repository visibility remains public.

## First release

1. Update `version` in Cargo.toml, run `cargo check --offline`, and describe
   user-visible changes in release notes. Keep README requirements and the Rust
   minimum accurate. Commit through a PR after branch protection is enabled.
2. Run `make test`, `make lint`, `make build`, and `make package`. Inspect
   `cargo package --offline --locked --list` to verify the distribution contains
   source, tests, README, and LICENSE without local files or credentials.
3. Wait for CI on the exact main commit. It validates Linux, macOS, and Rust
   1.74. Use `gh run list --repo joshmcadams/heads --workflow ci.yml --branch main`
   to choose that run. Download its binary artifacts into a fresh directory:

   ```sh
   gh run download RUN_ID --repo joshmcadams/heads --dir /tmp/heads-release
   ```

   The tarballs contain the binary, README, and MIT license. macOS binaries are
   ad hoc signed and are not notarized. Linux binaries use static CRT linking.
   CI produces binaries for the hosted runners' architectures, recorded in each
   filename; these artifacts do not cover every architecture.
4. Create a tag at that tested commit and push it. For the initial version:

   ```sh
   git tag -a v0.1.0 TESTED_COMMIT -m 'heads 0.1.0'
   git push origin v0.1.0
   ```

5. Rename tarballs to include the release version, generate SHA-256 checksums,
   and create a draft GitHub release with release notes and those assets:

   ```sh
   gh release create v0.1.0 --repo joshmcadams/heads --verify-tag --draft \
     --title 'heads 0.1.0' --notes-file /tmp/heads-release-notes.md \
     /tmp/heads-release/heads-0.1.0-*.tar.gz /tmp/heads-release/SHA256SUMS
   ```

   Review the draft, asset names, checksums, and installation instructions before
   publishing. Add the release downloads to README only after they exist.

## crates.io

The source package is prepared for Cargo publication, but no package has been
published to crates.io. Confirm that the name `heads` is available and that the
publishing account owns it before advertising `cargo install heads`. Authenticate
with `cargo login` outside repository files, then use `cargo publish --dry-run`
before `cargo publish`. A GitHub release and crates.io publication are separate
actions. Never commit a registry token.
