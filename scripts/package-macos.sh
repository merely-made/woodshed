#!/usr/bin/env bash
# Package a native Woodshed release binary as an architecture-specific macOS
# application bundle and checksummed ZIP. Signing and notarization are
# deliberately outside this unsigned-alpha script.

set -euo pipefail

usage() {
    echo "usage: $0 VERSION [OUTPUT_DIR]" >&2
    exit 2
}

die() {
    echo "package-macos: $*" >&2
    exit 1
}

[[ $# -ge 1 && $# -le 2 ]] || usage

version=$1
output_arg=${2:-dist}

if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]]; then
    die "invalid version '$version' (expected X.Y.Z or X.Y.Z-suffix)"
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
if [[ $output_arg == /* ]]; then
    output_dir=$output_arg
else
    output_dir=$root/$output_arg
fi

target_arg=${CARGO_TARGET_DIR:-target}
if [[ $target_arg == /* ]]; then
    target_dir=$target_arg
else
    target_dir=$root/$target_arg
fi

binary=$target_dir/release/woodshed-genet
[[ -f $binary && -x $binary ]] || die "release binary not found or not executable: $binary (run 'cargo build --release -p woodshed-genet --locked' first)"

arch=$(uname -m)
case $arch in
    arm64|x86_64) ;;
    *) die "unsupported macOS architecture '$arch'" ;;
esac

binary_arches=$(/usr/bin/lipo -archs "$binary" 2>/dev/null) || die "unable to inspect Mach-O architectures: $binary"
case " $binary_arches " in
    *" $arch ") ;;
    *) die "release binary architectures '$binary_arches' do not include host architecture '$arch'" ;;
esac

if [[ -n ${GITHUB_SHA:-} ]]; then
    source_revision=$GITHUB_SHA
else
    source_revision=$(git -C "$root" rev-parse HEAD) || die "unable to determine source revision"
fi

bundle_version=${version%%-*}
name="Woodshed-${version}-macos-${arch}"
archive_name=$name.zip
checksum_name=$archive_name.sha256
archive=$output_dir/$archive_name
checksum=$output_dir/$checksum_name

mkdir -p "$output_dir"
[[ ! -e $archive ]] || die "refusing to overwrite existing archive: $archive"
[[ ! -e $checksum ]] || die "refusing to overwrite existing checksum: $checksum"

work=$(mktemp -d "$output_dir/.woodshed-macos.XXXXXX")
cleanup() {
    rm -rf "$work"
}
trap cleanup EXIT INT TERM

stage=$work/$name
app=$stage/Woodshed.app
mkdir -p "$app/Contents/MacOS"

install -m 755 "$binary" "$app/Contents/MacOS/Woodshed"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>Woodshed</string>
    <key>CFBundleExecutable</key>
    <string>Woodshed</string>
    <key>CFBundleIdentifier</key>
    <string>made.merely.woodshed</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Woodshed</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$bundle_version</string>
    <key>CFBundleVersion</key>
    <string>$bundle_version</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.education</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Woodshed uses the microphone for the tuner and latency calibration when you enable them.</string>
</dict>
</plist>
EOF

for file in README.md LICENSE-MIT LICENSE-APACHE Cargo.lock; do
    [[ -f $root/$file ]] || die "required release file is missing: $root/$file"
    install -m 644 "$root/$file" "$stage/$file"
done

cat > "$stage/RELEASE-README.txt" <<EOF
Woodshed $version — macOS $arch unsigned alpha

Source revision: $source_revision
Signing: unsigned; no code-signing identity was applied
Notarization: none

Run Woodshed.app. Because this alpha is not signed or notarized, macOS may
require Control-click > Open the first time it is launched.

This alpha stores practice data in the local application configuration area.
Personae OS auto-unlock is not available on macOS in this lane. If no
passphrase-backed identity is supplied, Woodshed may use its unsealed fallback.
Do not use this build for sensitive practice data.

The source revision is recorded above. Cargo.lock and the project licenses are
included with this artifact; the repository source is not. This is a controlled
alpha package, not a Gatekeeper-trusted public release.
EOF

/usr/bin/plutil -lint "$app/Contents/Info.plist" >/dev/null || die "generated Info.plist is invalid"

archive_tmp=$work/$archive_name
(
    cd "$work"
    /usr/bin/ditto -c -k --sequesterRsrc --keepParent "$name" "$archive_tmp"
)

checksum_tmp=$work/$checksum_name
(
    cd "$work"
    /usr/bin/shasum -a 256 "$archive_name" > "$checksum_tmp"
)

mv "$archive_tmp" "$archive"
mv "$checksum_tmp" "$checksum"

echo "$archive"
echo "$checksum"
