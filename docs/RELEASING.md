# Releasing `ddog`

A release has **three** parts. Pushing a version tag alone is **not** enough — the
tag-push build will fail and nothing lands on crates.io. Do all three, in order.

The CLI is published as the crate **`bcl-ddog`** (the package in `crates/dd-cli`);
the binary is `ddog`. The workspace also publishes the library crates `dd-api`
and `dd-config`.

## 1. Bump the version

Versions are unified via `workspace.package.version`. Bump it **and** the two
path-dependency pins, or the build/publish will fail the semver match.

- `Cargo.toml` → `[workspace.package] version = "X.Y.Z"`
- `crates/dd-cli/Cargo.toml` → `dd-api` and `dd-config` `version = "X.Y.Z"`

Then refresh the lockfile and verify:

```sh
cargo build                       # updates Cargo.lock to X.Y.Z
cargo build --locked && cargo test --locked
./target/debug/ddog --version     # expect: ddog X.Y.Z
```

Commit, merge to `main`, then tag:

```sh
git tag -a vX.Y.Z -m "vX.Y.Z: <summary>" && git push origin vX.Y.Z
```

## 2. Attach the GitHub release binaries

> **Gotcha:** the tag-push run of `release.yml` fails with a retrying
> **`release not found`** loop. `upload-rust-binary-action` attaches to a GitHub
> *Release* object but doesn't create one (the workflow's `ref` is empty on push).
> This has failed on every tag-push release so far.

Create the Release first, then re-run the failed (already-compiled) jobs:

```sh
gh release create vX.Y.Z --title "vX.Y.Z — <summary>" --notes "..."
gh run rerun <run-id> --failed        # run-id from: gh run list --workflow=release.yml
```

Verify 5 targets uploaded (linux/macos x86_64+aarch64, windows x86_64) plus `.sha256`:

```sh
gh release view vX.Y.Z --json assets -q '.assets[].name'
```

(Equivalent alternative: trigger `release.yml` via `workflow_dispatch` with the
tag input — that path creates/uses the release correctly.)

## 3. Publish to crates.io (manual)

`release.yml` does **not** run `cargo publish`. Publish all three crates in
dependency order (each waits for the index before the next resolves):

```sh
cargo publish -p dd-config        # leaf
cargo publish -p dd-api
cargo publish -p bcl-ddog         # the CLI; pulls the two above from crates.io
```

## Downstream: bump the claudine layer

The sibling [`claudine`](https://github.com/Battle-Creek-LLC/claudine) repo has a
`ddog` dev-container layer that installs via `cargo binstall bcl-ddog@<ver>`
**from crates.io** — so it only works *after* step 3. To bump it:

- `src/layer.rs` → the `ddog` `Layer`'s `dockerfile`: `ARG DDOG_VERSION=X.Y.Z`
- `CHANGELOG.md` → the `[Unreleased]` `bcl-ddog@X.Y.Z` reference

## Known cleanup

- `release.yml` should create the Release itself (add a `gh release create` step or
  switch the action to create-on-publish) so step 2 stops needing a manual re-run.
- The `dependency-review` CI check fails on every PR until **Dependency graph** is
  enabled in repo Settings → Security & analysis.
