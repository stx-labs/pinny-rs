# Contributing

## Dependencies

Pinny is a proc-macro crate: every crate that uses it also compiles everything
Pinny depends on. Keep the dependency tree as small as possible.

- Prefer a few lines of standard-library code over a new crate. For example,
  `ConfigError` implements `Display` and `From` by hand rather than with
  `thiserror`, and tags are validated by `is_valid_tag` rather than with
  `regex`.
- When a crate is needed, disable its default features and enable only the
  ones in use. `toml` is built with just `parse`, which keeps `serde` out.
- The same applies to dev-dependencies. They do not reach consumers, but they
  still grow `Cargo.lock` and the set of crates to audit. For example, the
  config tests create their temporary directories with a small `TempDir`
  helper rather than with `tempfile`.
- Write tests so they can run in parallel: pass paths as arguments instead of
  setting environment variables, so no crate is needed to serialize them.
- A pull request that adds a dependency should explain why the standard
  library is not enough.

## Releasing

Releases are made from `main` by a maintainer, using the
[Release workflow](.github/workflows/release.yaml).

1. Open a pull request against `main` that sets the new `version` in
   `Cargo.toml`. Run `cargo check` so `Cargo.lock` is updated too, and commit
   both files: CI builds with `--locked` and fails on an outdated lock file.
2. Merge the pull request once CI passes.
3. In the repository's **Actions** tab, select **Release**, then
   **Run workflow**. Leave the branch on `main` and enter the version without
   the `v` prefix, for example `1.2.3`.

The workflow then:

- checks that the version has the form `X.Y.Z`;
- checks that `version` in `Cargo.toml` on `main` is the same version;
- tags the commit it checked as `vX.Y.Z`;
- creates the GitHub release `vX.Y.Z`, with notes generated from the merged
  pull requests.

If a check fails, nothing is created: fix the cause and run the workflow
again. If the tag already exists, the workflow stops without changing it. If
the tag was created but the release was not, either create the release for
that tag in the GitHub UI, or delete the tag and run the workflow again.
