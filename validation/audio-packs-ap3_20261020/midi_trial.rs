// SPDX-License-Identifier: MPL-2.0
//! Diagnostic artifact preparation, not a shipping instrument export profile.
//! Match the observed five-bar SMF envelope by repeating an app-authored bar.
use midly::{MetaMessage, Smf, TrackEvent, TrackEventKind};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 { return Err("usage: prep <app-export.mid> <new-trial.mid>".into()); }
    let bytes = std::fs::read(&args[0])?;
    let smf = Smf::parse(&bytes)?;
    if smf.header.format != midly::Format::SingleTrack || smf.header.timing != midly::Timing::Metrical(480.into()) { return Err("expected app SMF0 / PPQ480".into()); }
    let mut absolute = 0u32;
    let mut notes = Vec::new();
    for e in &smf.tracks[0] {
        absolute += e.delta.as_int();
        if let TrackEventKind::Midi {..} = e.kind { notes.push((absolute, e.kind)); }
    }
    if absolute != 1920 { return Err("expected one 4/4 app bar".into()); }
    let mut timeline = vec![(0, TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)))];
    for bar in 0..5u32 { for (tick, kind) in &notes { timeline.push((tick + bar*1920, *kind)); } }
    timeline.sort_by_key(|(tick,_)| *tick);
    let mut track = Vec::new();
    let mut previous = 0;
    for (tick,kind) in timeline { track.push(TrackEvent { delta:(tick-previous).into(), kind }); previous=tick; }
    track.push(TrackEvent { delta:(9600-previous).into(), kind:TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    let output = Smf { header:smf.header, tracks:vec![track] };
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&args[1])?;
    output.write_std(&mut file)?; file.sync_all()?;
    println!("Prepared SMF0 / PPQ480 / channel10, 5 bars, 60 notes, no tempo meta (guitar tempo controls playback).");
    Ok(())
}
