# Redshank playback spike

This unpublished workspace measures two decoder/transport paths while keeping
Firewheel as the sole audio-output owner:

- `redshank-symphonia-spike`: Symphonia decoding from local files or a bounded,
  seekable HTTP range source.
- `redshank-gstreamer-spike`: Genet's GStreamer player with a caller-supplied
  renderer feeding the same Firewheel output boundary.

The fixtures are generated locally and are not product assets. Each probe uses
mono 48 kHz input so Genet's current per-channel `AudioRenderer` callback can be
measured without treating repeated interleaved buffers as separate audio.

## Run

From this directory in PowerShell:

```powershell
./scripts/make-fixtures.ps1
cargo test -j 1
cargo run -p redshank-symphonia-spike -- target/fixtures/redshank.mp3 --max-seconds 1
cargo run -p redshank-symphonia-spike -- target/fixtures/redshank.m4a --max-seconds 1
cargo run -p redshank-gstreamer-spike -- target/fixtures/redshank.mp3 --max-seconds 1
cargo run -p redshank-gstreamer-spike -- target/fixtures/redshank.m4a --max-seconds 1
```

For the progressive seek probe, start the range server in one terminal:

```powershell
python ./scripts/range_server.py ./target/fixtures/redshank-episode.m4a 8765
```

Then run:

```powershell
cargo run -p redshank-symphonia-spike -- http://127.0.0.1:8765/redshank-episode.m4a --seek 450 --max-seconds 1
```

The probes require a default output device. The GStreamer probe additionally
requires the matching GStreamer development/runtime installation on `PATH` and
through `PKG_CONFIG_PATH` as expected by Genet.

## Done conditions

- local MP3 and AAC-LC reach a host-owned Firewheel graph through both paths;
- an HTTP episode seeks before the entire object is fetched;
- one output stream is opened per probe;
- deterministic press-time accounting stays within 50 ms;
- startup, binary size, dependency count, formats, and packaging constraints
  are recorded in the Redshank plan.
