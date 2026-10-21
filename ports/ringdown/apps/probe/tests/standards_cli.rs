// SPDX-License-Identifier: MPL-2.0
//! Exercise the real binary; virtual MIDI is confined to a receiving fixture.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ringdown-split-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn cli(args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Output {
    // Reuse the same cases against a preserved release artifact when requested.
    // The selected artifact must match this test binary's feature profile.
    let binary = std::env::var_os("RINGDOWN_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_ringdown-probe").into());
    Command::new(binary).args(args).output().unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn failure(output: Output, expected: &str) {
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(expected), "{error:?} lacks {expected:?}");
}

fn export(source: &Path, midi: &Path, saved: Option<&Path>) -> Output {
    let mut args = vec![
        OsStr::new("--audio-pack"),
        source.as_os_str(),
        OsStr::new("backbeat"),
        midi.as_os_str(),
    ];
    args.extend(saved.map(Path::as_os_str));
    cli(args)
}

fn inspect_backbeat(bytes: &[u8], bpm: u32) {
    use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
    let smf = Smf::parse(bytes).unwrap();
    assert_eq!(smf.header.format, Format::SingleTrack);
    assert_eq!(smf.header.timing, Timing::Metrical(480.into()));
    assert_eq!(smf.tracks.len(), 1);
    let mut tick = 0;
    let mut active = BTreeMap::<u8, u32>::new();
    let mut counts = BTreeMap::<u8, u32>::new();
    let mut tempo = None;
    let mut meter = None;
    let mut end = None;
    for event in &smf.tracks[0] {
        tick += event.delta.as_int();
        match event.kind {
            TrackEventKind::Midi { channel, message } => {
                assert_eq!(channel.as_int(), 9);
                match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        *active.entry(key.as_int()).or_default() += 1;
                        *counts.entry(key.as_int()).or_default() += 1;
                    },
                    MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, vel: _ } => {
                        let count = active.get_mut(&key.as_int()).unwrap();
                        assert!(*count > 0);
                        *count -= 1;
                    },
                    other => panic!("Unexpected beat message: {other:?}"),
                }
            },
            TrackEventKind::Meta(MetaMessage::Tempo(value)) => tempo = Some(value.as_int()),
            TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => meter = Some((n, d)),
            TrackEventKind::Meta(MetaMessage::EndOfTrack) => end = Some(tick),
            _ => {},
        }
    }
    assert_eq!(counts, BTreeMap::from([(36, 2), (38, 2), (42, 8)]));
    assert!(active.values().all(|n| *n == 0), "Unterminated notes");
    assert_eq!(tempo, Some(60_000_000 / bpm));
    assert_eq!(meter, Some((4, 2)));
    assert_eq!(end, Some(1920));
}

#[test]
fn default_entry_prints_help_without_starting_rpc() {
    let text = success(cli([] as [&str; 0]));
    assert!(text.contains("Beats MIDI-file export"));
    assert!(!text.contains("[1/4] scanning"));
}

#[test]
fn bundled_export_matches_shared_exporter_and_preserves_existing_output() {
    let scratch = Scratch::new();
    let midi = scratch.file("bundled.mid");
    let saved = scratch.file("saved.json");
    success(export(Path::new("bundled"), &midi, Some(&saved)));
    let bytes = std::fs::read(&midi).unwrap();
    let expected = audio_packs::files::load(Path::new("bundled"), Some("backbeat"))
        .unwrap()
        .midi()
        .unwrap();
    assert_eq!(bytes, expected.bytes);
    inspect_backbeat(&bytes, 100);
    let reopened = scratch.file("reopened.mid");
    success(export(&saved, &reopened, None));
    assert_eq!(bytes, std::fs::read(reopened).unwrap());
    failure(export(Path::new("bundled"), &midi, None), "exist");
    assert_eq!(bytes, std::fs::read(midi).unwrap());
}

#[test]
fn installed_pack_exports_and_reopens_after_installation_is_removed() {
    let scratch = Scratch::new();
    let installed = scratch.file("user-kit");
    std::fs::create_dir_all(installed.join("samples")).unwrap();
    let bundled =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../assets/audio-packs/basic-drums");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(bundled.join("audio-pack.json")).unwrap()).unwrap();
    manifest["id"] = "ringdown.split-test".into();
    manifest["patterns"]["backbeat"]["bpm"] = 120.0.into();
    std::fs::write(
        installed.join("audio-pack.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    for name in ["kick.wav", "snare.wav", "hat.wav"] {
        std::fs::copy(
            bundled.join("samples").join(name),
            installed.join("samples").join(name),
        )
        .unwrap();
    }
    let midi = scratch.file("installed.mid");
    let saved = scratch.file("retained.json");
    success(export(&installed, &midi, Some(&saved)));
    let bytes = std::fs::read(midi).unwrap();
    inspect_backbeat(&bytes, 120);
    std::fs::remove_dir_all(&installed).unwrap();
    let reopened = scratch.file("reopened.mid");
    success(export(&saved, &reopened, None));
    assert_eq!(bytes, std::fs::read(reopened).unwrap());
}

#[test]
fn rpc_feature_controls_routing_without_scanning() {
    let output = cli(["--rpc", "--help"]);
    if cfg!(feature = "hyvibe-rpc") {
        let text = success(output);
        assert!(text.contains("private HyVibe app-protocol diagnostic"));
        assert!(!text.contains("[1/4] scanning"));
        failure(cli(["--rpc", "--scan-secs", "invalid"]), "Nothing was sent");
    } else {
        failure(output, "Private RPC is excluded");
    }
}

#[test]
fn packetlogger_is_an_independent_feature() {
    let scratch = Scratch::new();
    let missing = scratch.file("missing.pklg");
    let output = cli([OsStr::new("--decode-pklg"), missing.as_os_str()]);
    failure(
        output,
        if cfg!(feature = "packetlogger") {
            "could not read"
        } else {
            "PacketLogger is excluded"
        },
    );
}

#[cfg(not(feature = "midi"))]
#[test]
fn offline_profile_refuses_midi_io() {
    failure(cli(["--midi-ports"]), "MIDI port I/O is excluded");
    failure(
        cli(["--midi-send", "anything", "1", "cc", "67", "127"]),
        "MIDI port I/O is excluded",
    );
}

#[cfg(feature = "midi")]
#[test]
fn malformed_midi_is_rejected_before_opening_a_port() {
    let missing = format!("Ringdown missing test port {}", std::process::id());
    for (args, expected) in [
        (vec!["0", "cc", "67", "127"], "between 1 and 16"),
        (vec!["17", "cc", "67", "127"], "between 1 and 16"),
        (vec!["1", "cc", "128", "127"], "between 0 and 127"),
        (vec!["1", "note-on", "60", "128"], "between 0 and 127"),
        (vec!["1", "cc", "67"], "require a value"),
        (vec!["1", "program", "7", "1"], "no value"),
        (vec!["1", "unknown", "7", "1"], "Type must be"),
    ] {
        let mut command = vec!["--midi-send", &missing];
        command.extend(args);
        failure(cli(command), expected);
    }
    failure(
        cli(["--midi-send", &missing, "1", "cc", "67", "127"]),
        "midi port not found",
    );
}

#[cfg(all(feature = "midi", any(target_os = "macos", target_os = "linux")))]
#[test]
fn real_cli_delivers_exact_midi_to_a_virtual_input() {
    use midir::{Ignore, MidiInput, os::unix::VirtualInput};
    use std::{sync::mpsc, time::Duration};
    let port = format!("Ringdown split capture {}", std::process::id());
    let (tx, rx) = mpsc::channel();
    let mut input = MidiInput::new("Ringdown test receiver").unwrap();
    input.ignore(Ignore::None);
    let _receiver = input
        .create_virtual(
            &port,
            move |_, bytes, _| {
                tx.send(bytes.to_vec()).unwrap();
            },
            (),
        )
        .unwrap();
    let ports = success(cli(["--midi-ports"]));
    assert!(ports.contains(&format!("output\t{port}")), "{ports}");
    for (args, expected) in [
        (vec!["1", "cc", "67", "127"], vec![0xb0, 67, 127]),
        (vec!["1", "cc", "67", "0"], vec![0xb0, 67, 0]),
        (vec!["16", "note-on", "127", "127"], vec![0x9f, 127, 127]),
        (vec!["16", "note-off", "127", "0"], vec![0x8f, 127, 0]),
        (vec!["1", "program", "0"], vec![0xc0, 0]),
        (vec!["16", "program", "127"], vec![0xcf, 127]),
    ] {
        let mut command = vec!["--midi-send", &port];
        command.extend(args);
        success(cli(command));
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), expected);
    }
}
