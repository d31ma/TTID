#!/bin/sh
# Run cargo through rustup's shim, so rust-toolchain.toml is honoured.
#
# Some machines have a package-manager cargo earlier on PATH (Homebrew's, for
# one). That binary is not a rustup shim, so it ignores rust-toolchain.toml and
# silently builds with whatever version it happens to be — which is how you get
# a green local build and a red CI one. Putting rustup's shim first fixes it.
#
#   ./scripts/cargo.sh test
#   ./scripts/cargo.sh clippy --all-targets -- -D warnings
#
# CI installs rustup normally and can call cargo directly; this is for humans.
set -eu
PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
export PATH
exec cargo "$@"
