#!/usr/bin/env bash
# Package Woodshed's native Linux alpha as an inspectable portable archive.
#
# This is deliberately not an installer: it neither bundles system libraries
# nor installs desktop metadata. See the generated RELEASE-README.txt for the
# runtime boundary the archive actually makes.

set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/package-linux.sh --version VERSION --revision GIT_SHA [--output-dir DIR]

Build `woodshed-genet` first with:
  cargo build --release -p woodshed-genet --locked

The script writes a portable x86_64 Linux tar.gz plus a SHA-256 sidecar. Paths
are relative to the repository root unless they are absolute paths under it.
USAGE
}

die() {
    printf 'package-linux: %s\n' "$*" >&2
    exit 1
}

version=''
revision=''
output_dir_input='dist'
seen_version=false
seen_revision=false
seen_output_dir=false

while (($#)); do
    case "$1" in
        --version)
            $seen_version && die '--version was supplied more than once'
            (($# >= 2)) || die '--version needs a value'
            version="$2"
            seen_version=true
            shift 2
            ;;
        --revision)
            $seen_revision && die '--revision was supplied more than once'
            (($# >= 2)) || die '--revision needs a value'
            revision="$2"
            seen_revision=true
            shift 2
            ;;
        --output-dir)
            $seen_output_dir && die '--output-dir was supplied more than once'
            (($# >= 2)) || die '--output-dir needs a value'
            output_dir_input="$2"
            seen_output_dir=true
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

$seen_version || die '--version is required'
$seen_revision || die '--revision is required'

# Keep release names interoperable with the existing Windows alpha convention,
# while rejecting whitespace, a leading v, and malformed prerelease labels.
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$ ]] || \
    die "version must be MAJOR.MINOR.PATCH with an optional prerelease: $version"
[[ "$revision" =~ ^[0-9a-f]{40}$ ]] || \
    die "revision must be one lowercase, full 40-character Git SHA: $revision"

[[ "$(uname -s)" == 'Linux' ]] || die 'this packager runs only on Linux'
[[ "$(uname -m)" == 'x86_64' ]] || die 'this packager currently supports only x86_64 Linux'

command -v realpath >/dev/null || die 'realpath is required to constrain package paths'
command -v sha256sum >/dev/null || die 'sha256sum is required'
command -v ldd >/dev/null || die 'ldd is required to record the build-host link snapshot'
command -v tar >/dev/null || die 'tar is required'
command -v gzip >/dev/null || die 'gzip is required'
command -v git >/dev/null || die 'git is required to verify the source revision'

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
current_revision="$(git -C "$root" rev-parse HEAD)"
[[ "$revision" == "$current_revision" ]] || \
    die "revision must match the checked-out HEAD ($current_revision)"

resolve_under_root() {
    local input="$1"
    local candidate
    local resolved
    if [[ "$input" == /* ]]; then
        candidate="$input"
    else
        candidate="$root/$input"
    fi
    resolved="$(realpath -m -- "$candidate")"
    case "$resolved" in
        "$root"/*)
            printf '%s\n' "$resolved"
            ;;
        *)
            die "path must resolve inside the repository: $input"
            ;;
    esac
}

output_dir="$(resolve_under_root "$output_dir_input")"
target_dir="$(resolve_under_root "${CARGO_TARGET_DIR:-target}")"
binary="$target_dir/release/woodshed-genet"

[[ -x "$binary" ]] || die "release binary is missing or not executable: $binary"
[[ -f "$root/README.md" ]] || die 'README.md is missing'
[[ -f "$root/LICENSE" ]] || die 'LICENSE is missing'
[[ -f "$root/Cargo.lock" ]] || die 'Cargo.lock is missing'

mkdir -p -- "$output_dir"

package_name="Woodshed-$version-linux-x86_64"
archive="$output_dir/$package_name.tar.gz"
checksum="$archive.sha256"
[[ ! -e "$archive" ]] || die "refusing to overwrite existing archive: $archive"
[[ ! -e "$checksum" ]] || die "refusing to overwrite existing checksum: $checksum"

stage_parent="$(mktemp -d "$output_dir/.woodshed-package.XXXXXX")"
stage="$stage_parent/$package_name"
cleanup() {
    rm -rf -- "$stage_parent"
}
trap cleanup EXIT
mkdir -- "$stage"

install -m 755 -- "$binary" "$stage/Woodshed"
install -m 644 -- "$root/README.md" "$stage/README.md"
install -m 644 -- "$root/LICENSE" "$stage/LICENSE"
install -m 644 -- "$root/Cargo.lock" "$stage/Cargo.lock"

binary_sha256="$(sha256sum -- "$stage/Woodshed" | awk '{print $1}')"
lock_sha256="$(sha256sum -- "$stage/Cargo.lock" | awk '{print $1}')"

cat >"$stage/RELEASE-README.txt" <<EOF
Woodshed $version — Linux x86_64 portable alpha

This archive is a portable alpha, not an installer. Extract it somewhere you
control and run ./Woodshed from the extracted directory. It does not install a
desktop entry, icon, service, package-manager record, or update mechanism.

Runtime boundary
----------------
This x86_64 glibc Linux build relies on your operating system's dynamic
libraries and desktop stack. In particular it needs Fontconfig, ALSA, a
graphical X11 or Wayland session, and a GPU driver usable by the selected WGPU
backend. It does not bundle, replace, or configure those system components.
Microphone access is requested only for tuner or latency-calibration use; MIDI
requires the system ALSA sequencer stack and compatible hardware.

RUNTIME-LIBRARIES.txt is an ldd snapshot from the build host. It is provenance
evidence only: it is not a distro-independent dependency contract and does not
list every display or graphics driver that Winit/WGPU may load dynamically.

Verify and run
--------------
From the directory containing this archive and its .sha256 sidecar:

  sha256sum -c $package_name.tar.gz.sha256
  tar -xzf $package_name.tar.gz
  cd $package_name
  sha256sum Woodshed
  ldd Woodshed
  ./Woodshed

The source revision, binary digest, and Cargo.lock digest are recorded in
PACKAGE-RECEIPT.txt. The source is not included; the root LICENSE contains the
project's MPL-2.0 terms, and the committed Cargo.lock is the dependency
inventory.
EOF

{
    printf 'Woodshed portable Linux package receipt\n'
    printf 'version: %s\n' "$version"
    printf 'source_revision: %s\n' "$revision"
    printf 'target: x86_64-unknown-linux-gnu\n'
    printf 'archive_format: portable tar.gz; not an installer\n'
    printf 'project_license: MPL-2.0\n'
    printf 'binary_sha256: %s\n' "$binary_sha256"
    printf 'cargo_lock_sha256: %s\n' "$lock_sha256"
} >"$stage/PACKAGE-RECEIPT.txt"

{
    printf '%s\n' 'Build-host dynamic-library evidence from ldd(1)'
    printf '%s\n' 'This sorted snapshot is not a portable runtime guarantee.'
    printf '%s\n' 'It omits libraries that the display and graphics stacks may load dynamically.'
    printf '\n'
    LC_ALL=C ldd "$stage/Woodshed" | LC_ALL=C sort
} >"$stage/RUNTIME-LIBRARIES.txt"

# GNU tar plus gzip -n gives the archive envelope normalized metadata. The
# executable itself is a normal Rust release build and is not claimed to be
# reproducible merely because the tar/gzip wrapper is stable.
archive_tmp="$stage_parent/$package_name.tar.gz"
checksum_tmp="$stage_parent/$package_name.tar.gz.sha256"

if tar --version 2>/dev/null | grep -Fq 'GNU tar'; then
    tar \
        --sort=name \
        --mtime="@${SOURCE_DATE_EPOCH:-0}" \
        --owner=0 \
        --group=0 \
        --numeric-owner \
        -C "$stage_parent" \
        -cf - "$package_name" \
        | gzip -n >"$archive_tmp"
else
    printf '%s\n' 'package-linux: GNU tar unavailable; archive metadata is not normalized' >&2
    tar -C "$stage_parent" -cf - "$package_name" | gzip -n >"$archive_tmp"
fi

archive_sha256="$(sha256sum -- "$archive_tmp" | awk '{print $1}')"
printf '%s  %s\n' "$archive_sha256" "$(basename -- "$archive")" >"$checksum_tmp"

mv -- "$archive_tmp" "$archive"
mv -- "$checksum_tmp" "$checksum"

printf '%s\n' "$archive"
printf '%s\n' "$checksum"
