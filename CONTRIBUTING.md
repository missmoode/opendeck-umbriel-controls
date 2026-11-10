# Contributing to Umbriel Controls

A quick reference for working on Umbriel Controls, including the checks run by CI and
how releases are managed. Basically for me when I come back to this after being away for
a while, because I'm forgetful, but if anyone else wants to contribute it's useful to
know.

## Getting started

**Nix is optional.** To develop without it, install a recent stable Rust toolchain
(including Cargo, rustfmt, and Clippy). You will also need `jq`, `zip`, and `unzip` if
you want to build and inspect a plugin archive.

```sh
rustup component add rustfmt clippy
cargo check --locked
cargo test --workspace --locked
```

The project uses Rust 2024 edition. An Umbriel session and OpenDeck are useful for
manual integration testing, but not required for most Rust checks and unit tests.

If you use Nix, the flake provides a development environment:

```sh
nix develop
```

You can then enter with a VSCode-based editor if you want to use the recommended .vscode
extensions and config:

```sh
codium .
```

## Checks before submitting a PR

These are the same Rust checks used by CI's **preflight** job:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked --workspace --all-targets -- -D warnings
```

For automatic Rust formatting, use `cargo fmt --all`. Run the tests too:

```sh
cargo test --workspace --locked
```

Clippy treats warnings as errors in CI. Keep `Cargo.lock` committed and in sync with the
manifests: builds use `--locked`, so changes that require a lockfile update will fail
unless that update is included. If a crate version changes, `cargo update --workspace`
can refresh workspace entries in the lockfile.

To build an installable archive locally on supported Linux hosts:

```sh
./build.sh --archive
unzip -t dist/*.streamDeckPlugin
```

Archives are placed in `dist/`. The build script uses the host architecture (`x86_64` or
`aarch64`) and requires `jq` and `zip`.

### Formatting outside Rust

The optional Nix formatter covers Markdown (`mdformat`), YAML (`yamlfmt`),
HTML/JavaScript/CSS/JSON (`prettier`), Rust (`rustfmt`), and shell scripts (`shfmt`).
Run it locally with:

```sh
nix fmt
```

`CHANGELOG.md`, the generated plugin manifest, and the bundled SDPI components are
excluded from relevant formatters. The CI workflow currently enforces Rust formatting
via `cargo fmt`, **not** the full `nix fmt` check. You do not need Nix to contribute.

## CI and pull requests

Open a PR against `main` with a brief explanation of the change and any manual testing
performed. The `Check and build` workflow runs on PRs and pushes to `main`:

1. **Rust checks:** `cargo fmt`, `cargo check --locked`, and workspace Clippy.
1. **Linux:** Workspace tests, an archive build, and archive/licence checks.
1. **Nix:** `nix flake check` and a Nix package build.

The Linux and Nix jobs start only after the Rust checks pass. Nix is available in CI, so
contributors do not need it installed locally.

## Commit messages and releases

Use [Conventional Commits](https://www.conventionalcommits.org/) so Release Please can
prepare version bumps and changelogs appropriately:

| Prefix | Use for | Normally appears in release notes? |
| -- | -- | -- |
| `feat:` | New user-facing functionality | Yes (minor bump) |
| `fix:` | User-facing bug fixes | Yes (patch bump) |
| `docs:` | Documentation-only changes | No |
| `ci:` | CI/workflow changes | No |
| `chore:` | Maintenance and tooling | No |
| `style:` | Formatting or cosmetic code changes | No |
| `refactor:` | Internal restructuring without a behaviour change | No |
| `test:` | Test-only changes | No |

Add a scope when useful, for example `fix(screencast): retain pinned target`. Use
`BREAKING CHANGE:` in the commit footer (or the `!` notation) for breaking changes. A
`Release-As: x.y.z` footer can explicitly request a release version, but should not be
used just to refresh a release PR.

Release Please manages crate versions, `Cargo.lock`, `assets/manifest.json`,
`CHANGELOG.md`, and the release PR. **Do not manually bump versions for routine
changes.** With the current configuration, a `docs:`, `chore:`, or `ci:` commit alone
may not regenerate an already-open release PR; don't label maintenance as `fix:` solely
to force an update.

When a release PR is merged, Release Please creates a **draft** GitHub release. The
release workflow builds archives for both `x86_64` and `aarch64`, attaches them, and
publishes the release only after the uploads succeed. Release Please handles the release
process; contributors only need to submit the changes.
