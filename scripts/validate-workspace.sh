#!/usr/bin/env bash
# The four products own separate lockfiles and test scopes. Resolve from a
# temporary directory so gitignored repository Cargo overrides cannot qualify
# a clean checkout accidentally.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <lockfile|test> <woodshed|hocket|redshank|ringdown>" >&2
  exit 2
fi

mode="$1"
workspace="$2"
root="$(cd "$(dirname "$0")/.." && pwd)"
case "$workspace" in
  woodshed)
    workspace_dir="$root"
    packages=(-p woodshedding -p audio-primitives -p timed-text
      -p woodshed-audio -p woodshed-core -p woodshed-graph
      -p woodshed-views -p woodshed-instrument -p woodshed-genet)
    ;;
  hocket)
    workspace_dir="$root/ports/hocket"
    packages=(-p hocket-model -p hocket-project -p hocket-engine -p hocket-genet
      -p hocket-headless)
    ;;
  redshank)
    workspace_dir="$root/ports/redshank"
    packages=(-p redshank-model -p redshank-storage -p redshank-cache
      -p redshank-feed -p redshank-playback -p redshank-surfaces
      -p redshank-desktop)
    ;;
  ringdown)
    workspace_dir="$root/ports/ringdown"
    packages=(--workspace)
    ;;
  *)
    echo "unknown workspace: $workspace" >&2
    exit 2
    ;;
esac

case "$mode" in
  lockfile|test) ;;
  *) echo "unknown validation mode: $mode" >&2; exit 2 ;;
esac

# Ports may carry their own toolchain; otherwise they use the family's root.
toolchain_file="$workspace_dir/rust-toolchain.toml"
if [[ ! -f "$toolchain_file" ]]; then
  toolchain_file="$root/rust-toolchain.toml"
fi
toolchain="$(sed -n 's/^channel = "\([^"]*\)".*/\1/p' "$toolchain_file")"
if [[ -z "$toolchain" ]]; then
  echo "missing toolchain channel in $toolchain_file" >&2
  exit 2
fi

validation_dir="$(mktemp -d "${TMPDIR:-/tmp}/woodshed-validation.XXXXXX")"
trap 'rm -rf "$validation_dir"' EXIT
cd "$validation_dir"
echo "$workspace: $mode (Rust $toolchain; outside repository Cargo config)"

if [[ "$mode" == lockfile ]]; then
  cargo "+$toolchain" metadata --manifest-path "$workspace_dir/Cargo.toml" \
    --locked --format-version 1 > /dev/null
else
  # Ignored hardware/fixture tests keep their explicit admission requirements.
  cargo "+$toolchain" test --manifest-path "$workspace_dir/Cargo.toml" \
    --locked "${packages[@]}"
fi
