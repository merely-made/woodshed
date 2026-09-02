//! The Phase 1 control run.
//!
//! Everything ringdown knows about the protocol was read out of a decompiled
//! application, which makes it a hypothesis. This is the experiment that tests
//! it: connect to a real guitar, read its version banner, ask for its status,
//! and print what came back next to what was predicted.
//!
//! A `GetStatus` that answers promotes the whole recovered map from
//! static-read to hardware-verified in one shot. A `GetStatus` that does not
//! answer is just as informative, and the output is written so the *first*
//! step that diverged is obvious rather than buried.
//!
//! Usage:
//!
//! ```text
//! ringdown-probe                       scan, connect, banner + status
//! ringdown-probe --config out.json     also dump the live effect catalog
//! ringdown-probe --write-len 244       override the assumed write length
//! ringdown-probe --scan-secs 20        scan for longer
//! ```

use std::time::Duration;

use ringdown_ble::{Guitar, MAX_FILE_CHUNK, MatchedBy, TransportError, discover};

struct Args {
    scan: Duration,
    write_len: Option<usize>,
    config_out: Option<String>,
    diagnose: bool,
    trace: bool,
    bank: Option<(i64, i64)>,
    call: Vec<(String, String)>,
    sweep: bool,
    dirs: bool,
    files: Option<String>,
    transport: Option<ringdown_ble::Transport>,
    timeout: Option<u64>,
    fetch: Option<String>,
    fetch_bytes: Option<usize>,
    index: bool,
}

/// Parse call parameters as JSON, or as comma-separated `key=value` pairs.
///
/// The `key=value` form exists because PowerShell strips the quotes out of
/// `{"bank_num":0}` before the program ever sees it, leaving `{bank_num:0}`,
/// which is not JSON. Rather than teach every user a shell-quoting rule,
/// accept a form that needs no quotes at all.
fn parse_params(text: &str) -> Result<serde_json::Value, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(serde_json::json!({}));
    }
    if text.starts_with('{') {
        // Tolerate the unquoted-key shape PowerShell produces.
        return serde_json::from_str(text).or_else(|_| {
            let repaired = repair_unquoted_keys(text);
            serde_json::from_str(&repaired)
                .map_err(|e| format!("not JSON ({e}); try key=value form instead, e.g. bank_num=0"))
        });
    }
    let mut map = serde_json::Map::new();
    for pair in text.split(',') {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("`{pair}` is not key=value"))?;
        let value = if let Ok(n) = v.trim().parse::<i64>() {
            serde_json::json!(n)
        } else if let Ok(f) = v.trim().parse::<f64>() {
            serde_json::json!(f)
        } else if v.trim() == "true" || v.trim() == "false" {
            serde_json::json!(v.trim() == "true")
        } else {
            serde_json::json!(v.trim())
        };
        map.insert(k.trim().to_string(), value);
    }
    Ok(serde_json::Value::Object(map))
}

/// Put quotes back around bare object keys, undoing what a shell removed.
fn repair_unquoted_keys(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if c == '"' {
            in_string = !in_string;
            out.push(c);
            continue;
        }
        if !in_string && (c.is_ascii_alphabetic() || c == '_') {
            let mut word = String::new();
            word.push(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    word.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            let is_key = matches!(chars.peek(), Some(':'));
            let literal = matches!(word.as_str(), "true" | "false" | "null");
            if is_key && !literal {
                out.push('"');
                out.push_str(&word);
                out.push('"');
            } else {
                out.push_str(&word);
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Accept either `3` or `0-7`.
fn parse_range(text: &str) -> Result<(i64, i64), String> {
    if let Some((a, b)) = text.split_once('-') {
        let first: i64 = a.trim().parse().map_err(|_| "range start isn't a number")?;
        let last: i64 = b.trim().parse().map_err(|_| "range end isn't a number")?;
        if last < first {
            return Err(String::from("range end is before its start"));
        }
        Ok((first, last))
    } else {
        let n: i64 = text.trim().parse().map_err(|_| "--bank isn't a number")?;
        Ok((n, n))
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        scan: Duration::from_secs(10),
        write_len: None,
        config_out: None,
        diagnose: false,
        trace: false,
        bank: None,
        call: Vec::new(),
        sweep: false,
        dirs: false,
        files: None,
        transport: None,
        timeout: None,
        fetch: None,
        fetch_bytes: None,
        index: false,
    };
    let mut argv = std::env::args().skip(1).peekable();
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--scan-secs" => {
                let v = argv.next().ok_or("--scan-secs needs a value")?;
                args.scan =
                    Duration::from_secs(v.parse().map_err(|_| "--scan-secs isn't a number")?);
            }
            "--write-len" => {
                let v = argv.next().ok_or("--write-len needs a value")?;
                args.write_len = Some(v.parse().map_err(|_| "--write-len isn't a number")?);
            }
            "--bank" => {
                let v = argv
                    .next()
                    .ok_or("--bank needs a number or range like 0-7")?;
                args.bank = Some(parse_range(&v)?);
            }
            "--call" => {
                let m = argv.next().ok_or("--call needs a method name")?;
                // Only take a following argument as params if it could be
                // params. Swallowing the next flag turns a valid command line
                // into a baffling error about an argument the user did write.
                let p = match argv.peek() {
                    Some(next) if !next.starts_with("--") => argv.next().unwrap(),
                    _ => String::from("{}"),
                };
                args.call.push((m, p));
            }
            "--sweep" => args.sweep = true,
            "--transport" => {
                let v = argv.next().ok_or("--transport needs llt or llt2")?;
                args.transport = Some(match v.as_str() {
                    "llt" => ringdown_ble::Transport::Llt,
                    "llt2" => ringdown_ble::Transport::Llt2,
                    other => return Err(format!("unknown transport: {other}")),
                });
            }
            "--timeout" => {
                let v = argv.next().ok_or("--timeout needs seconds")?;
                args.timeout = Some(v.parse().map_err(|_| "--timeout isn't a number")?);
            }
            "--fetch" => {
                args.fetch = Some(argv.next().ok_or("--fetch needs a path, or 'latest'")?);
            }
            "--fetch-bytes" => {
                let v = argv.next().ok_or("--fetch-bytes needs a number")?;
                args.fetch_bytes = Some(v.parse().map_err(|_| "--fetch-bytes isn't a number")?);
            }
            "--dirs" => args.dirs = true,
            "--index" => args.index = true,
            "--files" => {
                args.files = Some(argv.next().ok_or("--files needs a directory")?);
            }
            "--diagnose" => args.diagnose = true,
            "--trace" => args.trace = true,
            "--config" => {
                args.config_out = Some(argv.next().ok_or("--config needs a path")?);
            }
            "-h" | "--help" => {
                println!("{}", HELP);
                std::process::exit(0);
            }
            other => return Err(format!("unrecognised argument: {other}")),
        }
    }
    Ok(args)
}

const HELP: &str = "\
ringdown-probe — confirm the recovered HyVibe protocol against a real guitar

  --scan-secs <n>    how long to scan (default 10)
  --write-len <n>    override the assumed write length (default 514)
  --config <path>    also run ReadConfig and write the result here (wedges the
                     firmware until power-cycled, H18; the driver refuses it
                     unless the probe asks)
  --bank <n|a-b>     also run ReadBank for one bank or a range (e.g. 0-7)
  --call <m> [json]  call any method by wire name, including ones ringdown has
                     no variant for (e.g. --call PrintBank '{\"bank_num\":0}').
                     Repeatable. Anything arriving after the reply is reported.
  --sweep            ask the device which of the dictionary's undocumented
                     methods actually exist. Query-shaped methods only — the
                     ones that would change the instrument are never swept.
  --dirs             map the device filesystem by probing directory names.
                     Read-only: it only ever asks about files that do not exist.
  --files <dir>      guess filenames inside a directory, using the same oracle.
                     Read-only. e.g. --files /Calibration
  --transport <t>    force llt or llt2 rather than choosing by firmware version
  --timeout <secs>   how long to wait for a reply (default 10)
  --fetch <path>     download a recording off the device and save it. Use
                     'latest' for the most recent. Verified against the
                     device's own checksum.
  --fetch-bytes <n>  fetch only the first n bytes (skips the checksum, which
                     can only be verified over a whole file)
  --index            list every loop with its tempo and length, by reading only
                     each file's 92-byte header. Read-only, and cheap: a header
                     is one round trip where a whole loop is ten minutes.
  --trace            print every notification as it arrives
  --diagnose         if GetStatus is unanswered, try candidate encodings and
                     report which, if any, the device replies to
  -h, --help         this text

Turn the guitar on and make sure the phone app is NOT connected to it: the
instrument accepts one connection at a time, and the app will hold it.";

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            // Deliberately not the full help. It ends with advice about the
            // guitar's connection state, which reads as a diagnosis of the
            // instrument when the only thing wrong is the command line.
            eprintln!("error: {e}");
            eprintln!("Nothing was sent to the guitar. Run --help for usage.");
            std::process::exit(2);
        }
    };

    if let Err(e) = run(args).await {
        eprintln!("\nFAILED: {e}");
        eprintln!("\n{}", advice(&e));
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), TransportError> {
    println!("ringdown probe — Phase 1 control run");
    println!("everything below is predicted from static analysis until it answers\n");

    println!("[1/4] scanning {:?}...", args.scan);
    println!("      looking for service {}", ringdown::GUITAR_SERVICE);
    println!("      or a device named like the System Menu's BT ID (e.g. H2-SE614)");
    let found = discover(args.scan).await?;
    for (i, f) in found.iter().enumerate() {
        let how = match f.matched_by {
            MatchedBy::AdvertisedService => "advertises the guitar service",
            MatchedBy::Name => "name only — service not in the advertisement",
        };
        println!(
            "      found #{i}: {} [{}] — {how}",
            f.name.as_deref().unwrap_or("(unnamed)"),
            f.address
        );
    }
    if found[0].matched_by == MatchedBy::AdvertisedService {
        println!("      ADVERTISEMENT CONFIRMED — the service UUID is in the advertisement");
    } else {
        println!("      matched by name; whether the service exists is settled on connect");
    }

    println!("\n[2/4] connecting to #0...");
    let mut guitar = ringdown_ble::connect(&found[0]).await?;
    if let Some(len) = args.write_len {
        guitar.set_write_len(len);
    }
    guitar.set_trace(args.trace || args.diagnose);
    if let Some(secs) = args.timeout {
        guitar.set_request_timeout(Duration::from_secs(secs));
        println!("      reply timeout set to {secs}s");
    }
    if let Some(t) = args.transport {
        guitar.set_transport(t);
        println!("      transport FORCED to {t:?}");
    }
    println!("      connected; GATT surface has both expected characteristics");
    println!(
        "      write length in use: {} (assumed, not negotiated)",
        guitar.write_len()
    );
    println!(
        "      transport: {:?} (chosen from the firmware versions in the banner)",
        guitar.transport()
    );

    // Everything past this point runs inside `session`, so a failure still
    // releases the connection. The instrument accepts one client at a time, so
    // a leaked connection does not merely waste a handle: it locks out the next
    // run, and the symptom is some *other* request appearing to fail. An early
    // `?` here would make this probe the cause of the next probe's failure.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let outcome = session(&mut guitar, &args).await;

    let _ = guitar.disconnect().await;
    println!("\ndisconnected.");
    outcome
}

/// The work done while connected. The caller always disconnects afterwards,
/// whatever this returns.
async fn session(guitar: &mut Guitar, args: &Args) -> Result<(), TransportError> {
    println!("\n[3/4] reading the version banner (a GATT read, before any RPC)...");
    match guitar.banner().await {
        Ok(banner) => {
            println!("      BANNER CONFIRMED");
            println!(
                "      audio DSP (STM): {}{}",
                banner.stm,
                if banner.stm_was_implied {
                    "  <- assumed, not reported"
                } else {
                    ""
                }
            );
            println!("      connectivity (ESP): {}", banner.esp);
        }
        Err(e) => {
            println!("      banner did not parse: {e}");
            println!("      (continuing — the banner is not required for RPC)");
        }
    }

    if args.diagnose {
        return diagnose(guitar).await;
    }

    println!("\n[4/4] GetStatus — the control run...");
    let status = guitar.status().await?;
    println!("      *** THE PROTOCOL MAP IS CONFIRMED AGAINST HARDWARE ***\n");
    println!("      device          {}", status.device);
    println!("      cpu id          {}", status.cpu_id);
    println!("      battery         {:.0}%", status.battery_percent);
    println!(
        "      free space      {:.2} GB ({:.1}% free)",
        status.free_space_gb,
        status.free_space_fraction * 100.0
    );
    println!("      firmware ESP    {}", status.version_esp);
    println!("      firmware STM    {}", status.version_stm);
    println!("\n      cpu id and STM version are what a firmware assessment would start from.");

    if let Some((first, last)) = args.bank {
        println!(
            "
[extra] ReadBank {first}..={last}"
        );
        println!("        the device returns a *string* per bank, and the vendor's own app");
        println!("        never calls this, so its semantics are ours to establish.");
        let mut answered = 0;
        let mut nonempty = 0;
        for n in first..=last {
            match guitar
                .call(
                    ringdown::rpc::Method::ReadBank,
                    ringdown::rpc::params::bank(n),
                )
                .await
            {
                Ok(v) => {
                    answered += 1;
                    let rendered = match v.as_str() {
                        Some("") => String::from("(empty string)"),
                        Some(text) => {
                            nonempty += 1;
                            format!("{} chars: {text}", text.len())
                        }
                        None => {
                            nonempty += 1;
                            format!(
                                "not a string: {}",
                                serde_json::to_string(&v).unwrap_or_default()
                            )
                        }
                    };
                    println!("        bank {n}: {rendered}");
                    // A reply may be an acknowledgement with the payload
                    // following separately; check rather than assume.
                    let trailing = guitar.drain(Duration::from_millis(600)).await;
                    for line in &trailing {
                        println!("          + trailing: {line}");
                    }
                }
                Err(e) => println!("        bank {n}: {e}"),
            }
        }
        println!(
            "
        {answered} of {} answered, {nonempty} with content.",
            last - first + 1
        );
        if answered > 0 && nonempty == 0 {
            println!(
                "        Every bank answered but all were empty, so ReadBank is reachable
                         and either the banks are genuinely empty or their content lives
                         somewhere other than this result. Reachability is what this tested."
            );
        }
    }

    if args.sweep {
        sweep_methods(guitar).await;
    }

    if args.dirs {
        probe_directories(guitar).await;
    }

    if args.index {
        index_loops(guitar).await?;
    }

    if let Some(target) = args.fetch.as_deref() {
        fetch_recording(guitar, target, args.fetch_bytes).await?;
    }

    if let Some(dir) = args.files.as_deref() {
        probe_files(guitar, dir).await;
    }

    for (method, params_json) in &args.call {
        // Not a device method: `--call pause 5` waits five seconds before the
        // next call, holding the connection. Exists to separate "the write did
        // not apply" from "the write had not applied *yet*" — a distinction a
        // back-to-back write/read cannot see.
        if method == "pause" {
            let secs: u64 = params_json.trim().parse().unwrap_or(3);
            println!("\n[extra] pause — holding the connection {secs}s");
            tokio::time::sleep(Duration::from_secs(secs)).await;
            continue;
        }
        println!(
            "
[extra] {method} — an arbitrary method call"
        );
        let params = match parse_params(params_json) {
            Ok(v) => v,
            Err(e) => {
                println!("        bad params: {e}");
                continue;
            }
        };
        println!("        params: {params}");
        match guitar.call_named(method, params).await {
            Ok(v) => {
                println!(
                    "        ANSWERED: {}",
                    serde_json::to_string(&v).unwrap_or_default()
                );
                if let Some(dump) = render_maybe_hex(&v) {
                    print!("{dump}");
                }
            }
            Err(e) => println!("        {e}"),
        }
        let trailing = guitar.drain(Duration::from_millis(800)).await;
        if trailing.is_empty() {
            println!("        (nothing followed)");
        } else {
            for line in &trailing {
                println!("        + trailing: {line}");
            }
        }
    }

    if let Some(path) = args.config_out.as_deref() {
        println!("\n[extra] ReadConfig — the live effect catalog...");
        println!("        this is the test of how an over-MTU response arrives (Finding F11).");
        println!("        WARNING: ReadConfig wedges this firmware's RPC handler (H18); expect");
        println!("        to power-cycle the guitar afterwards. The driver refuses it by default.");
        guitar.allow_wedging_calls();
        let config = guitar
            .call(
                ringdown::rpc::Method::ReadConfig,
                ringdown::rpc::params::none(),
            )
            .await?;
        let pretty = serde_json::to_string_pretty(&config)
            .unwrap_or_else(|_| String::from("<unserializable>"));
        match std::fs::write(path, &pretty) {
            Ok(()) => println!("        wrote {} bytes to {path}", pretty.len()),
            Err(e) => println!("        could not write {path}: {e}"),
        }
    }

    Ok(())
}

/// Stems worth trying inside a directory, and the extensions to pair them with.
///
/// Drawn from the compressor's keyword dictionary and from the one filename
/// convention actually observed — `/Loops/loop0031.wav`, a lowercase word with
/// a zero-padded index — rather than from general intuition about filenames.
const FILE_STEMS: &[&str] = &[
    "calibration",
    "cal",
    "calib",
    "analysis",
    "resonance",
    "resonances",
    "filter",
    "filters",
    "fbk",
    "feedback",
    "notch",
    "biquad",
    "biquads",
    "config",
    "conf",
    "settings",
    "data",
    "guitar",
    "params",
    "speaker",
    "spk",
    "cal0001",
    "calibration0001",
    "analysis0001",
    "0001",
    "0",
    "1",
];

const FILE_EXTS: &[&str] = &["json", "dat", "bin", "txt", "cfg", "cal", ""];

/// Guess filenames inside a directory, using the same oracle as `--dirs`.
///
/// A hit returns real metadata rather than an error, so unlike the directory
/// sweep this one can find something outright. Still read-only: `GetFileInfo`
/// reports size and checksum and changes nothing.
async fn probe_files(guitar: &mut Guitar, dir: &str) {
    const DIR_EXISTS: f32 = 4.0;
    const DIR_MISSING: f32 = 5.0;

    let dir = dir.trim_end_matches('/');
    println!(
        "
[extra] filename probe in {dir}"
    );
    println!("        read-only; a hit returns size and checksum.");

    let mut hits = Vec::new();
    let mut saw_dir = false;
    let mut tried = 0usize;

    for stem in FILE_STEMS {
        for ext in FILE_EXTS {
            let name = if ext.is_empty() {
                format!("{dir}/{stem}")
            } else {
                format!("{dir}/{stem}.{ext}")
            };
            tried += 1;
            match guitar
                .call_named("GetFileInfo", serde_json::json!({ "name": name }))
                .await
            {
                Ok(v) => {
                    saw_dir = true;
                    println!(
                        "        FOUND {name} -> {}",
                        serde_json::to_string(&v).unwrap_or_default()
                    );
                    hits.push(name);
                }
                Err(TransportError::Rpc(ringdown::rpc::RpcError::Device(e))) => {
                    if e.code == DIR_EXISTS {
                        saw_dir = true;
                    } else if e.code != DIR_MISSING {
                        println!("        {name}: unexpected code {} ({})", e.code, e.message);
                    }
                }
                Err(e) => {
                    println!("        {name}: {e}");
                    return;
                }
            }
        }
    }

    println!();
    if !saw_dir {
        println!("        CONTROL FAILED: every probe said the directory is absent, but");
        println!("        {dir} was reported present. Treat this run as meaningless.");
        return;
    }
    println!("        {tried} names tried, {} found.", hits.len());
    if hits.is_empty() {
        println!("        The directory is there and holds none of these names. Either its");
        println!("        contents follow a convention not guessed here, or it is empty.");
    }
}

/// Pull a recording off the instrument and write it to disk.
///
/// The vendor offers this only over USB mass storage, so it is one of the
/// things a desktop client earns outright. It is slow: replies are hex, which
/// doubles every byte, and one reply carries about two hundred bytes of file.
/// A multi-megabyte loop is therefore minutes rather than seconds, which is
/// why progress is reported rather than left to a silent wait.
/// List every loop with its tempo, reading only each file's header.
///
/// The point is the cost. `DumpFile` takes an offset and a size, so a loop's
/// metadata is one round trip of 92 bytes where its audio is roughly 3,700 round
/// trips and ten minutes. Indexing a whole library is therefore seconds, and
/// browsing by tempo is practical even though bulk retrieval is not.
///
/// It also settles an open question. Two of the header's six values multiply to
/// the loop's length in beats, and one file cannot say which is bars and which
/// is beats-per-bar. Several files can: the field that stays at 4 across loops
/// in common time is the meter, and the one that moves is the bar count.
async fn index_loops(guitar: &mut Guitar) -> Result<(), TransportError> {
    println!(
        "
[extra] indexing loops by header"
    );

    let next = guitar.next_recording_name().await?;
    let Some((stem, count, ext)) = split_numbered(&next) else {
        println!("        could not read a loop number out of {next}");
        return Ok(());
    };
    if count == 0 {
        println!("        the device reports no recordings");
        return Ok(());
    }
    println!("        device names the next loop {next}, so {count} exist at most");
    println!();
    println!("        file             samples   nominal    delta  bpm beats  raw values");

    let mut seen: Vec<ringdown::loopfile::LoopMeta> = Vec::new();
    let mut disagreed = 0usize;

    for n in 1..count {
        let name = format!("{stem}{n:04}{ext}");
        let bytes = match guitar
            .read_file_range(&name, 0, ringdown::loopfile::HEADER_PREFIX)
            .await
        {
            Ok(bytes) => bytes,
            // A gap in the numbering is an answer, not a failure.
            Err(TransportError::Rpc(ringdown::rpc::RpcError::Device(_))) => continue,
            Err(e) => return Err(e),
        };

        let short = name.rsplit('/').next().unwrap_or(&name).to_string();
        match ringdown::loopfile::parse(&bytes) {
            Ok(header) => {
                let meta = header.meta;
                // Exact sample counts, not seconds: the question is whether a
                // loop lands on its nominal length or misses it, and rounding
                // to two decimals hides differences of a few hundred samples.
                let samples = i64::from(header.samples());
                let (bpm, music) = match meta {
                    Some(m) => (
                        format!("{}", m.tempo_bpm),
                        format!(
                            "{}/{} x{:<2} {}",
                            m.beats_per_bar,
                            m.beat_unit,
                            m.bars,
                            if m.is_partial() { "partial" } else { "" }
                        ),
                    ),
                    None => ("-".into(), "(no vendor chunk)".into()),
                };
                let (nominal, delta) =
                    match meta.and_then(|m| m.nominal_samples(header.format.sample_rate)) {
                        Some(n) => (format!("{n}"), format!("{:+}", samples - n as i64)),
                        None => ("-".into(), "-".into()),
                    };
                println!(
                    "        {short:<15} {samples:>8} {nominal:>9} {delta:>8} \
                     {bpm:>4}  {music}"
                );
                if header.length_agrees() == Some(false) {
                    disagreed += 1;
                    println!("          ^ DISAGREES with the model — worth more than the fits");
                }
                if let Some(m) = meta {
                    seen.push(m);
                }
            }
            Err(e) => println!("        {short:<15} unreadable: {e}"),
        }
    }

    summarise_library(&seen, disagreed);
    Ok(())
}

/// Summarise the library, and say whether the header model held.
///
/// The field meanings are settled (see `ringdown::loopfile`), so this is a
/// check rather than a discovery: every complete take should land on its
/// block-rounded grid length, and every partial one should fall short of it. A
/// disagreement means this instrument writes something the model does not
/// cover, and is the only line here worth acting on.
fn summarise_library(seen: &[ringdown::loopfile::LoopMeta], disagreed: usize) {
    println!();
    if seen.is_empty() {
        println!("        no loop carried the vendor's chunk.");
        return;
    }

    let distinct = |f: fn(&ringdown::loopfile::LoopMeta) -> u32| {
        let mut v: Vec<u32> = seen.iter().map(f).collect();
        v.sort_unstable();
        v.dedup();
        v
    };

    let mut meters: Vec<String> = seen
        .iter()
        .map(|m| format!("{}/{}", m.beats_per_bar, m.beat_unit))
        .collect();
    meters.sort();
    meters.dedup();
    let partial = seen.iter().filter(|m| m.is_partial()).count();

    println!("        across {} loops:", seen.len());
    println!("          time signatures  {}", meters.join(", "));
    println!("          tempos           {:?}", distinct(|m| m.tempo_bpm));
    println!("          bar counts       {:?}", distinct(|m| m.bars));
    println!("          format versions  {:?}", distinct(|m| m.version));
    println!(
        "          {partial} partial take(s), {} complete",
        seen.len() - partial
    );
    println!();

    if disagreed == 0 {
        println!("        every loop's audio matches what its header implies.");
    } else {
        println!("        {disagreed} loop(s) DISAGREE with the header model. That is worth");
        println!("        more than all the agreements: the model was fitted to one");
        println!("        instrument's library, and this is how it gets corrected.");
    }
}

/// Split `/Loops/loop0032.wav` into `("/Loops/loop", 32, ".wav")`.
fn split_numbered(path: &str) -> Option<(&str, u32, &str)> {
    let dot = path.rfind('.')?;
    let (base, ext) = path.split_at(dot);
    let digits_start = base.len()
        - base
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .count();
    if digits_start == base.len() {
        return None;
    }
    let number = base[digits_start..].parse().ok()?;
    Some((&base[..digits_start], number, ext))
}

async fn fetch_recording(
    guitar: &mut Guitar,
    target: &str,
    partial: Option<usize>,
) -> Result<(), TransportError> {
    println!(
        "
[extra] fetching a recording"
    );

    let name = if target == "latest" {
        match guitar.latest_recording_name().await? {
            Some(name) => {
                println!("        latest existing recording: {name}");
                name
            }
            None => {
                println!("        the device reports no recording that can be opened");
                return Ok(());
            }
        }
    } else {
        target.to_string()
    };

    let info = guitar.file_info(&name).await?;
    println!(
        "        {name}: {} bytes, device checksum {:#010x}",
        info.size, info.crc32
    );

    if let Some(n) = partial {
        // A range cannot be checksummed, so this is for looking rather than
        // for keeping, and says so.
        let bytes = guitar
            .read_file_range(&name, 0, n.min(MAX_FILE_CHUNK))
            .await?;
        println!(
            "        first {} bytes (checksum not verifiable on a range):",
            bytes.len()
        );
        print_hexdump(&bytes);
        return Ok(());
    }

    let started = std::time::Instant::now();
    let mut last_report = 0u64;
    let bytes = guitar
        .read_file(&name, MAX_FILE_CHUNK, |done, total| {
            // Report on every whole percent rather than every chunk, which
            // would be thousands of lines.
            let pct = done * 100 / total.max(1);
            if pct != last_report {
                last_report = pct;
                eprint!("\r        {pct}% ({done}/{total} bytes)");
            }
        })
        .await?;
    eprintln!();

    println!(
        "        CHECKSUM VERIFIED — {} bytes in {:.1}s",
        bytes.len(),
        started.elapsed().as_secs_f64()
    );

    let out = name.rsplit('/').next().unwrap_or("recording.wav");
    match std::fs::write(out, &bytes) {
        Ok(()) => println!("        wrote {out}"),
        Err(e) => println!("        could not write {out}: {e}"),
    }
    print_hexdump(&bytes[..bytes.len().min(64)]);
    Ok(())
}

/// Print bytes as hex and ASCII, so a WAV header identifies itself.
fn print_hexdump(bytes: &[u8]) {
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!(
            "          {:04x}  {:<47}  |{ascii}|",
            row * 16,
            hex.join(" ")
        );
    }
}

/// Directory names worth asking about.
///
/// `/Loops` is first and is not a guess: it is the positive control, known to
/// exist because its files have been read. A run where it does not report as
/// existing has a broken oracle, and every other line of output is worthless.
const DIR_CANDIDATES: &[&str] = &[
    "/Loops",
    "/Config",
    "/config",
    "/Banks",
    "/banks",
    "/Presets",
    "/presets",
    "/Effects",
    "/effects",
    "/System",
    "/system",
    "/Settings",
    "/settings",
    "/Data",
    "/data",
    "/User",
    "/Factory",
    "/Firmware",
    "/firmware",
    "/Recordings",
    "/Analysis",
    "/Calibration",
    "/log",
    "/logs",
    "/tmp",
    // Second pass: ESP32 filesystem mount points, and more product-shaped
    // names. The first pass found /Calibration, so the naming is capitalised
    // and domain-flavoured rather than unix-ish, but both are cheap to ask.
    "/Cal",
    "/Analysis",
    "/Audio",
    "/Rec",
    "/Records",
    "/Music",
    "/Sounds",
    "/Samples",
    "/Bank",
    "/Preset",
    "/Effect",
    "/Fx",
    "/FX",
    "/Profile",
    "/Profiles",
    "/Guitar",
    "/Setup",
    "/Backup",
    "/IR",
    "/spiffs",
    "/littlefs",
    "/sd",
    "/sdcard",
    "/flash",
    "/nvs",
    "/etc",
    "/usr",
    "/var",
    "/home",
    "/root",
];

/// A filename chosen to be absent everywhere, so the probe only ever asks about
/// files that do not exist and never touches real content.
const ABSENT_FILE: &str = "zzz_ringdown_probe_absent.tmp";

/// Map the filesystem by asking about files that are not there.
///
/// Confirmed against controls on 2026-08-27: `GetFileInfo` answers **4** when
/// the directory exists but the file does not, and **5** when the directory
/// itself is missing. That difference makes a missing-file query into a
/// directory-existence test, which is the only enumeration this protocol
/// offers — there is no listing method.
///
/// Entirely read-only. Every request names a file designed not to exist.
async fn probe_directories(guitar: &mut Guitar) {
    const DIR_EXISTS: f32 = 4.0;
    const DIR_MISSING: f32 = 5.0;

    println!(
        "
[extra] filesystem probe — which directories exist?"
    );
    println!("        read-only: every query names a file designed not to exist.");
    println!("        code 4 = directory present, code 5 = directory absent.");

    let mut found = Vec::new();
    let mut control_ok = false;

    for dir in DIR_CANDIDATES {
        let path = format!("{dir}/{ABSENT_FILE}");
        let verdict = match guitar
            .call_named("GetFileInfo", serde_json::json!({ "name": path }))
            .await
        {
            Err(TransportError::Rpc(ringdown::rpc::RpcError::Device(e))) => {
                if e.code == DIR_EXISTS {
                    if *dir == "/Loops" {
                        control_ok = true;
                    }
                    found.push(*dir);
                    String::from("EXISTS")
                } else if e.code == DIR_MISSING {
                    String::from("absent")
                } else {
                    format!("unexpected code {} ({})", e.code, e.message)
                }
            }
            // A file that was supposed to be absent is not: still proof the
            // directory is there, and worth saying out loud.
            Ok(v) => {
                found.push(*dir);
                format!(
                    "EXISTS — and the probe file somehow does too: {}",
                    serde_json::to_string(&v).unwrap_or_default()
                )
            }
            Err(e) => format!("{e}"),
        };
        println!("        {dir:<14} {verdict}");
    }

    println!();
    if !control_ok {
        println!("        CONTROL FAILED: /Loops did not report as existing, though its");
        println!("        files have been read successfully. The oracle is not working on");
        println!("        this run, so treat every line above as meaningless.");
        return;
    }
    println!("        control passed (/Loops reports as existing).");
    println!("        directories found: {}", found.join(", "));
    if found.len() == 1 {
        println!("        Only the control. Either the configuration is not stored as a");
        println!("        file, or its directory is not among the names guessed here.");
    }
}

/// Render a hex-string reply as bytes, since some methods answer that way.
///
/// `DumpFile` returns file contents as an uppercase hex string. Printed raw it
/// is an unreadable wall; decoded, a WAV header or a JSON fragment identifies
/// itself at a glance. Anything that is not plausibly hex is left alone.
fn render_maybe_hex(value: &serde_json::Value) -> Option<String> {
    let text = value.as_str()?;
    if text.len() < 8 || text.len() % 2 != 0 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Vec<u8> = (0..text.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect();
    if bytes.len() * 2 != text.len() {
        return None;
    }

    let mut out = format!(
        "        decoded {} bytes:
",
        bytes.len()
    );
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        out.push_str(&format!(
            "          {:04x}  {:<47}  |{ascii}|
",
            row * 16,
            hex.join(" ")
        ));
    }
    Some(out)
}

/// Methods the compressor's dictionary names but the vendor's app never calls,
/// restricted to those whose names promise only to *report* something.
///
/// The mutating remainder — `ActivateSpkFilter`, `BypassEffect`, `DumpFile`,
/// `LaunchCalibration`, `PullFbk`, `SetGainPreamp`, `SetPhaseInv`,
/// `SetSpeakerBiquads`, `StartAnalysis`, `StartRendering`, `StartTuner`,
/// `StopRendering`, `StopTuner` — is deliberately absent. Sweeping those would
/// mean firing thirteen state-changing commands with guessed parameters at
/// someone's instrument to satisfy curiosity. They are one `--call` away when
/// there is a reason to want one.
const QUERY_METHODS: &[&str] = &[
    "BTcheck",
    "GetAnalysis",
    "GetLastRecordingName",
    "GetLevels",
    "PrintBank",
    "ReadMetronome",
];

/// Ask the device which methods it implements.
///
/// The keyword dictionary proves the firmware knows a string; it does not prove
/// there is a method behind it. The device settles that itself — an unknown
/// method comes back as error 4, "Method not found", which makes this a real
/// membership test rather than a guess.
async fn sweep_methods(guitar: &mut Guitar) {
    println!(
        "
[extra] method sweep — which undocumented methods exist?"
    );
    println!("        query-shaped methods only; nothing here changes the instrument.");

    let mut exists = Vec::new();
    let mut missing = Vec::new();

    for name in QUERY_METHODS {
        match guitar.call_named(name, serde_json::json!({})).await {
            Ok(v) => {
                println!(
                    "        {name}: EXISTS — {}",
                    serde_json::to_string(&v).unwrap_or_default()
                );
                exists.push(*name);
            }
            Err(TransportError::Rpc(ringdown::rpc::RpcError::Device(e))) => {
                // Code 4 is the device's "Method not found"; any other code
                // means the method is real and merely objected to the call.
                if e.code == 4.0 {
                    println!("        {name}: not implemented");
                    missing.push(*name);
                } else {
                    println!(
                        "        {name}: EXISTS — but rejected: {} ({})",
                        e.message, e.code
                    );
                    exists.push(*name);
                }
            }
            Err(e) => println!("        {name}: {e}"),
        }
        let trailing = guitar.drain(Duration::from_millis(400)).await;
        for line in &trailing {
            println!("          + trailing: {line}");
        }
    }

    println!(
        "
        {} implemented, {} absent.",
        exists.len(),
        missing.len()
    );
    if !exists.is_empty() {
        println!("        implemented: {}", exists.join(", "));
        println!("        A method that exists but rejected an empty call wants parameters;");
        println!("        its error message is usually the best clue about which.");
    }
}

/// Try each candidate request encoding and report which the device answers.
///
/// The recovered encoding is a hypothesis, and when it goes unanswered the
/// useful next step is not to guess again but to test the small number of
/// things it could plausibly be, one at a time, and say what happened to each.
async fn diagnose(guitar: &mut Guitar) -> Result<(), TransportError> {
    const LISTEN: Duration = Duration::from_secs(4);

    println!(
        "
[4/4] diagnostic mode — the recovered encoding went unanswered
"
    );

    println!("  first: does the device ever speak unprompted?");
    let idle = guitar.listen(LISTEN).await;
    if idle.is_empty() {
        println!(
            "    nothing in {LISTEN:?} — expected; it should only answer when asked.
"
        );
    } else {
        println!(
            "    it sent {} message(s) with no request: {idle:?}
",
            idle.len()
        );
    }

    // Each candidate differs from the recovered encoding in exactly one way, so
    // whichever answers names the single wrong assumption.
    let candidates: [(&str, String, bool); 5] = [
        (
            "recovered: numeric jsonrpc, write-with-response",
            r#"{"jsonrpc":2.0,"id":90,"method":"GetStatus","params":{}}"#.to_string(),
            true,
        ),
        (
            "same, but write-WITHOUT-response",
            r#"{"jsonrpc":2.0,"id":91,"method":"GetStatus","params":{}}"#.to_string(),
            false,
        ),
        (
            "newline-terminated (as LLT frames are)",
            "{\"jsonrpc\":2.0,\"id\":92,\"method\":\"GetStatus\",\"params\":{}}
"
            .to_string(),
            true,
        ),
        (
            "spec-compliant STRING jsonrpc (contradicts F12)",
            r#"{"jsonrpc":"2.0","id":93,"method":"GetStatus","params":{}}"#.to_string(),
            true,
        ),
        (
            "no params field at all",
            r#"{"jsonrpc":2.0,"id":94,"method":"GetStatus"}"#.to_string(),
            true,
        ),
    ];

    let mut answered = Vec::new();
    for (label, payload, with_response) in &candidates {
        println!("  trying: {label}");
        println!("    -> {} bytes: {payload:?}", payload.len());
        match guitar.write_raw(payload.as_bytes(), *with_response).await {
            Ok(()) => {
                let heard = guitar.listen(LISTEN).await;
                if heard.is_empty() {
                    println!(
                        "    silence.
"
                    );
                } else {
                    println!(
                        "    ANSWERED with {} message(s)
",
                        heard.len()
                    );
                    answered.push((*label, heard));
                }
            }
            Err(e) => println!(
                "    the write itself failed: {e}
"
            ),
        }
    }

    println!(
        "
=== diagnosis ==="
    );
    if answered.is_empty() {
        println!(
            "No encoding drew a reply, and the device never spoke unprompted.
             That points away from the message format and toward the channel itself:
             - notifications may not really be enabled (the CCCD write may not have taken),
             - or writes are not reaching the device despite appearing to succeed, which
               is what an unnegotiated MTU would do to a 55-byte write.
             Next: confirm with a generic BLE tool (nRF Connect) that writing this exact
             payload to the request characteristic produces a notification at all."
        );
    } else {
        println!("These encodings drew a reply:");
        for (label, heard) in &answered {
            println!("  - {label}");
            for line in heard {
                println!("      {line:?}");
            }
        }
        println!(
            "
The one that answered names the assumption to correct in Findings."
        );
    }

    Ok(())
}

/// Turn a failure into the next thing worth trying.
///
/// A probe that fails is still an experiment with a result; what makes it
/// useful is saying which assumption it falsified.
fn advice(e: &TransportError) -> &'static str {
    match e {
        TransportError::NotFound(_) => {
            "Nothing in range looked like a guitar.\n\
             \n\
             Most likely it is simply asleep. These power down on their own, and a\n\
             session of repeated connections drains the battery noticeably. Tap the\n\
             knob and check the screen lights before reading further.\n\
             \n\
             If it is definitely awake:\n\
             The scan is unfiltered and matches on the service UUID *or* the name, so\n\
             this is not a scan-filter false negative. Check, in order:\n\
             - Is it powered on, and is USB mode OFF? USB mode makes it a mass-storage\n\
               drive, not a Bluetooth peripheral.\n\
             - Is the phone app connected? It holds the one connection the guitar has.\n\
             - Is it paired as a Bluetooth *speaker* in the OS? That is Bluetooth Classic\n\
               audio, a different radio path from the app's BLE control link. Unpair it.\n\
             - Check the System Menu for the BT ID (e.g. H2-SE614). If the name does not\n\
               start with H2- and does not contain 'hyvibe', tell ringdown what to match."
        }
        TransportError::NoAdapter => "No Bluetooth adapter. Is Bluetooth switched on?",
        TransportError::MissingCharacteristic(_) => {
            "Connected, but the characteristic UUIDs are not what was recovered.\n\
             This falsifies Finding F1 — the map is wrong or this is different firmware.\n\
             Dump the real GATT table and record it before going further."
        }
        TransportError::Timeout { heard, .. } if heard.is_empty() => {
            "The write succeeded and the device said nothing at all.\n\
             \n\
             If this request worked on an earlier run, suspect device state before the\n\
             code: a previous run that ended in an error may have left the instrument\n\
             mid-transfer, and it serves one client at a time. Power-cycle the guitar\n\
             (hold the knob two seconds, then tap it) and try again — a stuck transfer\n\
             shows up as some *other* request failing, which is a misleading symptom.\n\
             \n\
             Note that ReadConfig is known to wedge this firmware's RPC handler: after\n\
             one, every later request is met with silence until the guitar is power-\n\
             cycled, GetStatus included. If a ReadConfig was attempted at any point in\n\
             this session, that is almost certainly what you are looking at, and the\n\
             failing request is a victim rather than a cause.\n\
             \n\
             Otherwise, run:\n\
             \n\
                 cargo run -p ringdown-probe -- --diagnose\n\
             \n\
             which tries each candidate encoding in turn and reports which, if any, it\n\
             answers — including whether it ever speaks unprompted at all."
        }
        TransportError::Timeout { .. } => {
            "The device replied, but with nothing this client recognised as the answer.\n\
             That is much closer in than silence: the channel works and the device is\n\
             talking. The messages it sent are printed in the error above — compare them\n\
             against the expected envelope and correct the Findings accordingly."
        }
        TransportError::ChunkRejected { .. } => {
            "The device rejected a frame of a split message, which means it IS speaking\n\
             LLT — good — but our framing is off. The status code names which invariant\n\
             it disliked (sequence, object id, or the frame itself)."
        }
        TransportError::Rpc(_) => {
            "The device answered and the envelope parsed, so the transport is right.\n\
             This is a protocol-level disagreement about one method — much closer in\n\
             than a silent failure. Record the exact error."
        }
        TransportError::Link(_) => {
            "The Bluetooth stack refused the operation. If this was 'Not connected' during
             connect, the peripheral handle from the scan had gone stale, or the guitar
             dropped between being seen and being connected to — ringdown now retries
             that a few times, so a failure here means it failed repeatedly.
             
             Check the guitar is awake, then try again. If Windows has gotten confused,
             toggling Bluetooth off and on clears its device cache."
        }
        _ => "Record what happened in the plan's Findings before changing anything.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The device names the *next* recording, so the index counts up to it
    /// rather than including it. Getting this split wrong reads a file that
    /// does not exist, which is how `GetLastRecordingName` misled us once
    /// already.
    #[test]
    fn a_recording_path_splits_into_stem_number_and_extension() {
        assert_eq!(
            split_numbered("/Loops/loop0032.wav"),
            Some(("/Loops/loop", 32, ".wav"))
        );
        assert_eq!(
            split_numbered("/Loops/loop0001.wav"),
            Some(("/Loops/loop", 1, ".wav"))
        );
        // Leading zeros must not be read as octal, and the count is what the
        // digits say rather than how many there are.
        assert_eq!(split_numbered("/x/y0009.wav"), Some(("/x/y", 9, ".wav")));
    }

    #[test]
    fn a_path_with_no_number_is_declined_rather_than_guessed() {
        assert_eq!(split_numbered("/Loops/loop.wav"), None);
        assert_eq!(split_numbered("noextension0001"), None);
        assert_eq!(split_numbered(""), None);
    }

    /// Digits in the directory must not be mistaken for the file's number.
    #[test]
    fn only_the_digits_immediately_before_the_extension_count() {
        assert_eq!(
            split_numbered("/Loops2024/take0007.wav"),
            Some(("/Loops2024/take", 7, ".wav"))
        );
    }
}
