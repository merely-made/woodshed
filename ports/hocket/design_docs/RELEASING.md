# Releasing Hocket

The pipeline is Rust end to end: [`cargo-packager`](https://crates.io/crates/cargo-packager)
builds the installer, [`luggage`](https://github.com/merely-made/mere/tree/main/crates/system/luggage)
(mere) checks the feed and applies the update. No .NET, on any host.

Verified on Windows 2026-07-24 (see the auto-update plan's H4 receipts).

## Once per machine

```sh
cargo install cargo-packager --locked
cargo install --path <mere>/crates/system/luggage --bin luggage-manifest
```

## Once, ever: the release key

```sh
cargo packager signer generate --path <somewhere outside every repo>/hocket.key
```

Keep the private key offline and out of every repo. The `.pub` beside it is
the `HOCKET_UPDATE_PUBKEY` value shipped to clients; it is already in the
base64 form luggage wants.

## Per release, per host

Hosts pack their own format: `nsis` on Windows, `app` on macOS, `appimage`
on Linux. Each host adds its entry to the same `luggage.json`.

```sh
# 1. Bump the version in crates/hocket-genet/Cargo.toml.

# 2. Pack + sign. The manifest's before-packaging-command rebuilds first —
#    see the traps below for why that is not optional.
export CARGO_PACKAGER_SIGN_PRIVATE_KEY=<path to hocket.key>
export CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD=
cargo packager -p hocket-genet --release --formats nsis   # or app / appimage

# 3. Stage the artifact + its .sig in the feed, then add this host's entry.
luggage-manifest --artifact <feed>/hocket-genet_<ver>_x64-setup.exe \
                 --version <ver> --format nsis
```

`luggage-manifest` computes the BLAKE3 digest, reads the `.sig`, and merges
into an existing `luggage.json` when the version matches — so the Mac and
Linux entries can be added later without redoing the Windows one.

### 4. Sign the manifest itself — not optional

```sh
cargo packager signer sign <feed>/luggage.json     # writes luggage.json.sig
```

Do this **last**, after every host has added its entry, because any later
edit invalidates the signature.

The artifact signature proves the bytes are ours. It says nothing about the
*version and URL announced around them*, so without a signed manifest anyone
who controls the feed can advertise an old signed build as a new version and
roll clients backwards onto a release with a known flaw. Updaters therefore
refuse an unsigned manifest by default (`require_signed_manifest`), and
`luggage-manifest` prints the signing command as a reminder.

### Per-host artifact shapes

What luggage downloads is not always what `cargo packager` emits:

| Host | Pack format | Artifact the manifest points at |
| --- | --- | --- |
| Windows | `nsis` | the `…-setup.exe` as packed |
| Linux | `appimage` | the `.AppImage` itself (luggage writes the downloaded bytes straight over the running AppImage) |
| macOS | `app` | the **`.app.tar.gz`** the packer emits beside the bundle (luggage gunzips and untars) — *not* the `.app` itself |

macOS is the one to watch: `--formats app` writes both `Hocket.app` and a
signed `Hocket.app.tar.gz`. Install the bundle, but point the manifest at
the tarball:

```sh
cargo packager -p hocket-genet --release --formats app
luggage-manifest --artifact target/release/Hocket.app.tar.gz \
                 --version <ver> --format app
```

(An earlier version of this file claimed the tarball had to be made by hand,
inferred from the format docs. Observed behaviour on 2026-07-24: the packer
already produces and signs it.)

For a directory feed the default `file://` URL is right. For a published
feed pass `--url` with the URL the artifact will actually live at (for a
GitHub release, `https://github.com/<owner>/<repo>/releases/download/v<ver>/<file>`),
and upload `luggage.json` as a release asset so `github:owner/repo` finds it.

## Running the update

```sh
export HOCKET_UPDATE_FEED=<dir | https://… | github:owner/repo>
export HOCKET_UPDATE_PUBKEY=$(cat hocket.key.pub)
hocket-genet --update-now                 # or just launch the app
```

The policy is a stored device setting now, not an environment variable:
`update-settings.json` holds `policy` (`off` | `notify` | `download-then-ask` |
`automatic`), `channel`, and `check_interval_secs`. `HOCKET_UPDATE_POLICY` is
no longer read. To run against an isolated policy without touching the real
file, point `HOCKET_SETTINGS` at a scratch path:

```sh
export HOCKET_SETTINGS=/tmp/hocket-release-check/update-settings.json
```

`--update-now` runs the flow in the terminal and prints each real state; the
app does the same in the background and shows it in the top bar. Both honour
the policy, so `notify` reports an available version without fetching it.

## Working across the release machines

`Cargo.lock` is committed (correct for an application), but each machine
re-resolves it — a dev box has the gitignored `.cargo/config.toml` patches,
a clean one does not. So the lock is usually dirty on the secondary hosts,
and `git pull` **aborts**:

```text
Please commit your changes or stash them before you merge.
Aborting
```

The dangerous part is that a `git pull … && cargo build …` chain then
builds the *old* commit and fails in a way that looks like your fix did not
work. Restore the generated lock before pulling, and check the commit you
actually got:

```sh
git restore Cargo.lock && git pull --ff-only && git log --oneline -1
```

### Linux: two env vars this box needs

Fedora 44 (and anything else without libfuse2, or with a newer libxml2 than
linuxdeploy's bundled `strip` understands):

```sh
export APPIMAGE_EXTRACT_AND_RUN=1   # no libfuse2: AppImage tools cannot self-mount
export NO_STRIP=1                   # linuxdeploy's strip chokes on libxml2's .relr.dyn
```

Both are needed for *packing*, not just for running the result — the packer
runs linuxdeploy and appimagetool, which are themselves AppImages.

## Traps, both found the hard way on 2026-07-24

1. **`cargo packager` does not build your app.** `--release` only tells it
   where to look for a binary. Without a `before-packaging-command` it will
   package whatever is already in `target/release`.
2. **`cargo build` does not rebuild when only the version changed.** A
   version bump with no source change leaves the old binary in place, so the
   installer is *named* for the new version while the binary inside still
   reports the old one through `env!("CARGO_PKG_VERSION")` — and an update
   to it looks like it silently did nothing.

Both are handled by the `before-packaging-command` in
`crates/hocket-genet/Cargo.toml`, which cleans that one crate and rebuilds
before packing. Do not "simplify" it away.

**After packing, check the binary matches the label:**

```sh
./target/release/hocket-genet --update-now | head -1   # must print the new version
```

## macOS: Gatekeeper (a different job from the signatures above)

The minisign signatures prove the update bytes and the manifest are ours.
Code signing and notarization are what stop macOS warning the user at
install. Both are wanted; neither replaces the other.

The signing identity is in `crates/hocket-genet/Cargo.toml`
(`[package.metadata.packager.macos] signing-identity`) and must match
`security find-identity -v -p codesigning` on the signing Mac exactly.
Packing then signs automatically.

**Cut macOS releases in Terminal.app on the Mac, not over SSH.** `codesign`
needs the login keychain, and an SSH session can neither unlock it nor
prompt you, so it fails with:

```text
errSecInternalComponent            # from codesign
User interaction is not allowed.   # from `security`
```

That is macOS working as designed, not a misconfiguration. `~/mac-release.sh`
on the Mac does the whole run (identity check, pack, notarize, staple,
verify); the first run asks permission to use the signing key — choose
**Always Allow**. Automating this later means a dedicated CI keychain with
the certificate imported from a `.p12` and unlocked from a secret, not this
machine's login keychain.

Notarization needs credentials, which are secrets and so come from the
environment. Store them once per machine:

```sh
xcrun notarytool store-credentials "hocket-notary" \
  --apple-id "<apple id>" --team-id "<TEAMID>" --password "<app-specific-password>"
```

Then pack with `APPLE_KEYCHAIN_PROFILE=hocket-notary` set (or
`APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID`).

**Notarization uploads to Apple and waits for a verdict — minutes of
silence is normal.** Do not pipe the pack through `tail`/`head` while
waiting: they buffer until the command exits, so a working notarization
looks exactly like a hung one (it fooled us on 2026-07-26; `ps` showed
`notarytool` alive and working the whole time). Let the output stream.

**A first submission from a new Developer ID can sit "In Progress" for
hours.** Apple holds uploads it does not recognise for deeper analysis, and
says the wait shrinks as a team notarizes more. Ours took over an hour on
the first try with no verdict. This is not a failure and there is nothing
to fix; treat it as asynchronous:

- The submission processes **on Apple's side**. Interrupting `notarytool`
  only kills the local wait — the upload keeps going, which is how we ended
  up with two queued submissions of the same app. Harmless, but do not
  re-submit expecting to unstick it.
- No re-pack is needed later. The `.app` is already signed; a ticket is
  attached afterwards with `xcrun stapler staple`, or by re-running
  `~/mac-notary-finish.sh`, which waits, staples and verifies.
- `notarytool log <id>` reporting "not yet available" means *no verdict
  yet*, not a rejection. Do not read a missing staple as a rejection either
  (we did, briefly): check `notarytool info <id>` for the actual status.

Verify the result the way Gatekeeper will:

```sh
codesign -dvv Hocket.app          # identity + hardened runtime
spctl -a -vv Hocket.app           # "accepted", source=Notarized Developer ID
```

An app that is signed but *not* notarized still trips Gatekeeper on another
Mac, so `spctl` is the check that matters — not `codesign` alone.

## What is not automated yet

- Windows Authenticode: a purchasing decision (Azure Trusted Signing, or an
  EV certificate). Unsigned still installs; users meet SmartScreen.
- Publishing the feed (upload to GitHub Releases / a host) — the acceptance
  runs used local directory feeds.
- One signing key in one place: the per-host acceptance keypairs are a test
  artefact, since a client trusts exactly one public key.
