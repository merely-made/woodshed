// SPDX-License-Identifier: MPL-2.0
//! Shared pack export and standard MIDI commands. Private diagnostics are opt-in.

#[cfg(feature = "packetlogger")]
mod pklg;
#[cfg(feature = "hyvibe-rpc")]
mod rpc;

use std::{ffi::OsString, process::ExitCode};

const HELP: &str = "\
ringdown-probe — Beats MIDI-file export and standard MIDI control

  --audio-pack <source> <pattern> <new.mid> [new-rhythm.json]
      Use bundled, a pack directory, or a saved snapshot/rhythm JSON.
      Writes new local files through the shared audio-packs exporter.
  --midi-ports
      List OS MIDI input and output ports (requires the midi feature).
  --midi-send <exact-output-name> <channel 1-16> <type> <number> [value]
      Send one owner-configured message. Types: cc, note-on, note-off, program.
      Number and value must be 0-127; program takes no value.
      This does not assign guitar bindings or confirm an instrument action.
  --ble-midi [name] [--unencrypted-compatibility]
      Advertise a persistent MIDI pedal; encrypted by default. Commands on stdin:
      status | reopen | send <channel> <cc|note-on|note-off|program> <number> [value] | quit
  --rpc [options]
      Private app-protocol probe (requires the hyvibe-rpc feature).
      Use --rpc --help for its options.
  --decode-pklg <file>
      Private capture diagnostic (requires the separate packetlogger feature).
  -h, --help

MIDI uses OS ports or the explicit BLE peripheral output. BLE pairing and the
advertised device name depend on the platform; Windows uses its system name.
MIDI-file export does not copy files to the guitar or control Beats playback.";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        },
    }
}

fn dispatch(args: &[OsString]) -> Result<(), String> {
    let Some(command) = args.first().and_then(|arg| arg.to_str()) else {
        if args.is_empty() {
            println!("{HELP}");
            return Ok(());
        }
        return Err("Command must be UTF-8".into());
    };
    match command {
        "-h" | "--help" if args.len() == 1 => {
            println!("{HELP}");
            Ok(())
        },
        "--audio-pack" => audio_packs::files::run_cli(&args[1..]).map_err(|e| e.to_string()),
        "--midi-ports" | "--midi-send" => midi_command(command, &args[1..]),
        "--ble-midi" => ble_command(&args[1..]),
        "--rpc" => {
            #[cfg(feature = "hyvibe-rpc")]
            {
                rpc::main();
                Ok(())
            }
            #[cfg(not(feature = "hyvibe-rpc"))]
            Err("Private RPC is excluded from this build; it requires --features hyvibe-rpc".into())
        },
        "--decode-pklg" => {
            #[cfg(feature = "packetlogger")]
            {
                if args.len() != 2 {
                    return Err("Usage: --decode-pklg <file>".into());
                }
                let path = args[1].to_str().ok_or("Capture path must be UTF-8")?;
                pklg::decode_file(path)
            }
            #[cfg(not(feature = "packetlogger"))]
            Err(
                "PacketLogger is excluded from this build; it requires --features packetlogger"
                    .into(),
            )
        },
        _ => Err("Unrecognized command or arguments; run --help for usage".into()),
    }
}

#[cfg(not(feature = "midi"))]
fn midi_command(_command: &str, _args: &[OsString]) -> Result<(), String> {
    Err("MIDI port I/O is excluded from this build; it requires --features midi".into())
}

#[cfg(feature = "midi")]
fn midi_command(command: &str, args: &[OsString]) -> Result<(), String> {
    use woodshed_instrument::midi::{MidiOut, list_input_ports, list_output_ports};

    if command == "--midi-ports" {
        if !args.is_empty() {
            return Err("Usage: --midi-ports".into());
        }
        match list_input_ports() {
            Ok(names) => {
                for name in names {
                    println!("input\t{name}");
                }
            },
            Err(error) => eprintln!("OS MIDI inputs unavailable: {error}"),
        }
        for name in list_output_ports().map_err(|e| e.to_string())? {
            println!("output\t{name}");
        }
        return Ok(());
    }
    let (port, bytes) = parse_midi_send(args)?;
    // Validate the complete command before opening a port or sending bytes.
    let mut output = MidiOut::connect_by_name(&port).map_err(|e| e.to_string())?;
    output.send_raw(&bytes).map_err(|e| e.to_string())?;
    println!("MIDI {bytes:02x?} accepted by output {port:?}; instrument action unconfirmed");
    Ok(())
}

#[cfg(feature = "midi")]
fn parse_midi_send(args: &[OsString]) -> Result<(String, Vec<u8>), String> {
    if !(4..=5).contains(&args.len()) {
        return Err("Usage: --midi-send <exact-output-name> <channel 1-16> <cc|note-on|note-off|program> <number 0-127> [value 0-127]".into());
    }
    let text = |i: usize| args[i].to_str().ok_or("MIDI arguments must be UTF-8");
    let bounded = |i: usize, min: u8, max: u8| -> Result<u8, String> {
        text(i)?
            .parse::<u8>()
            .ok()
            .filter(|n| (min..=max).contains(n))
            .ok_or_else(|| format!("Argument {} must be between {min} and {max}", i + 1))
    };
    let port = text(0)?;
    use woodshed_instrument::pedal::{Binding, MessageKind};
    let channel = bounded(1, 1, 16)?;
    let kind = text(2)?;
    let number = bounded(3, 0, 127)?;
    let message_kind = match kind {
        "cc" => MessageKind::ControlChange,
        "note-on" => MessageKind::NoteOn,
        "note-off" => MessageKind::NoteOff,
        "program" => MessageKind::ProgramChange,
        _ => return Err("Type must be cc, note-on, note-off or program".into()),
    };
    let value = if kind == "program" {
        if args.len() != 4 {
            return Err("Program Change takes a number and no value".into());
        }
        0
    } else {
        if args.len() != 5 {
            return Err("CC and Note messages require a value/velocity".into());
        }
        bounded(4, 0, 127)?
    };
    let mut messages = Binding {
        enabled: true,
        channel,
        kind: message_kind,
        number,
        value,
        release: None,
    }
    .messages()?;
    Ok((port.to_string(), messages.remove(0)))
}

#[cfg(not(feature = "ble-midi"))]
fn ble_command(_args: &[OsString]) -> Result<(), String> {
    Err("BLE-MIDI peripheral is excluded from this build; it requires --features ble-midi".into())
}

#[cfg(feature = "ble-midi")]
fn ble_command(args: &[OsString]) -> Result<(), String> {
    use std::io::BufRead;
    use woodshed_instrument::midi::{
        MidiOut,
        ble::{Config, Security},
    };
    let mut config = Config::default();
    let mut name_set = false;
    for arg in args {
        if arg == "--unencrypted-compatibility" {
            config.security = Security::UnencryptedCompatibility;
        } else if !name_set && !arg.to_string_lossy().starts_with('-') {
            config.name = arg.to_str().ok_or("BLE name must be UTF-8")?.into();
            name_set = true;
        } else {
            return Err("Usage: --ble-midi [name] [--unencrypted-compatibility]".into());
        }
    }
    let mut output = MidiOut::advertise_ble(config.clone()).map_err(|error| error.to_string())?;
    println!(
        "BLE-MIDI {:?}; commands: status | reopen | send <channel> <type> <number> [value] | quit",
        output.ble_status()
    );
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        let words: Vec<_> = line.split_whitespace().collect();
        match words.first().copied() {
            Some("status") if words.len() == 1 => println!("BLE-MIDI {:?}", output.ble_status()),
            Some("reopen") if words.len() == 1 => {
                output.close().map_err(|error| error.to_string())?;
                output =
                    MidiOut::advertise_ble(config.clone()).map_err(|error| error.to_string())?;
                println!(
                    "BLE-MIDI reopened {:?}; no MIDI replayed",
                    output.ble_status()
                );
            },
            Some("send") => {
                let args: Vec<OsString> = std::iter::once(OsString::from("BLE-MIDI"))
                    .chain(words[1..].iter().map(OsString::from))
                    .collect();
                match parse_midi_send(&args).and_then(|(_, bytes)| {
                    output.send_raw(&bytes).map_err(|error| error.to_string())
                }) {
                    Ok(()) => println!("MIDI accepted by OS; musical result unobserved"),
                    Err(error) => eprintln!("Message refused/uncertain: {error}"),
                }
            },
            Some("quit") if words.len() == 1 => break,
            None => {},
            _ => eprintln!(
                "Commands: status | reopen | send <channel> <type> <number> [value] | quit"
            ),
        }
    }
    output.close().map_err(|error| error.to_string())
}
