# Provenance

This spike is independently written for Redshank and contains no copied donor
application code or media.

References consulted for API shape:

- Genet `components/media/player` and `components/media/backends/gstreamer`,
  MPL-2.0, for its public player/backend and caller-renderer contracts.
- Firewheel 0.10 stream-writer API and Woodshed sibling Hocket's Firewheel host
  integration, MIT OR Apache-2.0 upstream and MPL-2.0 for Hocket source.
- Symphonia 0.5.5 examples and public decode/source APIs, MPL-2.0.

FFmpeg generates the two synthetic sine-wave fixtures at test time. The
fixtures are excluded from source control. Termusic source was not consulted or
copied.
