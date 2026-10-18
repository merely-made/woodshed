# USB storage and Beats inspection, 2026-10-18

Status: bounded inspection complete. The owner enabled USB mode and authorized
read-only investigation. No application-initiated file or configuration writes
were made to the guitar. The drive was safely ejected at closeout.

## Observed transport

On Q-PC (Intel macOS 15.8.1), the USB-C–to–USB-C connection enumerated as
`MCU MASS STORAGE`, vendor `NXP SEMICONDUCTORS`, VID/PID `1fc9:0093`.
The active configuration exposed a mass-storage interface: class 8, subclass 6,
protocol 80, two endpoints. The volume `HYVIBE` was FAT32 on an approximately
7.8 GB device. No new MIDI endpoint or serial port appeared in this mode.
This does not establish other boot modes, other platforms or USB control.
The descriptor's manufacturer does not identify the guitar's DSP architecture.

The preceding USB-A setup did not enumerate, including after USB ON/restart.
Cable and computer connector changed together; the cause was not isolated.
USB-C–to–USB-C works for this observed guitar/host combination.

## Files and configuration

[observation.json](observation.json) retains the scoped interface facts, field
observations, file names, sizes and hashes. Root entries included:

- `config_user.json` (27,050 bytes): nine user banks and metronome settings;
- `config_device.json` (6,526 bytes): device/calibration settings and MIDI bindings;
- `Drums/`: ten `.mid` files;
- `Loops/`: 44 `.wav` files;
- `Calibration/`: two `.wav` files, not modified or copied.

The user configuration's metronome object was:

```json
{"drums_loop":true,"dl_filename":"6-Blues.mid","bpm":100,"num":4,"den":4,"nbbars":12}
```

These are stored configuration fields, not proof of live playback, Bluetooth
parameter acceptance or `nbbars` behavior. File schema versions were user 3
and device 2; these are not processor firmware versions. No `GetStatus` ran.

Drum files were `1-Rock.mid`, `2-Funky_16th.mid`, `3-Funky_8th.mid`,
`4-Shuffle_Groove.mid`, `5-Shuffle_Rock.mid`, `6-Blues.mid`,
`7-Pure_Swing.mid`, `8-Jazz_Waltz.mid`, `9-Bossa_Nova.mid`, `10-Salsa.mid`.
All ten headers had `MThd`, header length 6, format 0, one track and division
480. This is header inspection, not full event validation. Note maps, pattern
lengths, loading/naming rules, replacement/addition acceptance and persistence
remain unqualified. The original pattern bytes are not redistributed here.

The device file recorded active MIDI, paired name `Ringdown`, and a binding
with channel 1, type `Ctrl Chge`, value 67, action 4, consistent with the
owner's Effect Next setup from the preceding pedal session. This is one
file-observed action identifier, not a complete MIDI action mapping.

Private originals of the two configuration files were saved outside Git in
`/Users/markik/Code/testing/ringdown/captures/2026-10-18_usb/`, with restrictive
permissions and hashes. Original configurations, hardware addresses and audio
recordings are not included in this public receipt. USB snapshots provide
an offline source without invoking the guarded Bluetooth `ReadConfig`; they
do not automatically match Ringdown's shadow-profile representation.

## Permission, mount and recovery sequence

The investigator first unmounted/remounted the volume read-only. Directory
opening then waited on macOS removable-volume permission for Codex. During
that wait the storage disk disappeared while its USB interface remained.
The pending read returned EIO; the owner confirmed power and USB ON. The
read-only remount's role in this loss is unisolated.

After permission was handled, the owner toggled USB OFF/ON. The disk returned.
The normal read/write mount was left unchanged; the investigator used only
file reads. No further remount, repair, formatting or firmware operation ran.
System-created metadata directories were present; this receipt does not claim
that the operating system itself made no writes. The device was safely ejected.

## Consequences for the performance plan

- Use portable file I/O for explicit USB configuration snapshots and recording
  access; qualify platform mounting/admission and snapshot-to-model validation.
- Investigate recovered `drums_loop`/`dl_filename` fields through controlled
  panel/app evidence before adding any RPC write. JSON shape on disk does not
  establish an RPC envelope.
- Assess a guitar-native custom MIDI-pattern workflow before assuming custom
  rhythms require computer-generated audio. File existence alone does not prove
  that a new authored pattern is accepted. A persistent-device-write experiment
  needs the owner's explicit approval and a reviewable artifact first.

No production code, builds or tests were added or run in this USB inspection.
