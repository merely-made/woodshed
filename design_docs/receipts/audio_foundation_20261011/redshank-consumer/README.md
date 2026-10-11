# Redshank 0.14 consumer native regression

Built and replayed integration source `7d44ab1`, using standalone Firewheel
`2b03846d2e0dcc6ded2a85367bb8c5071ae99cdc`. The real desktop binary was copied
into three finite LaunchServices applications. All three returned `RESULT ok`
without timeout; the twelve presented-frame PNGs were reviewed and are nonblank.

The lifecycle run visits Listen, Library, Notes and Settings. The appearance
run refuses unsaved Apply, saves/applies a user theme, and renders all four
modes. A fresh process restores that saved theme and high-contrast dark mode.
Listener, appearance and library files were isolated from the user's profile.
This is native host regression and appearance persistence evidence. Playback,
microphone permission, audible output, mixed-rate physical devices, Turnstone
and the standalone repository extraction remain separate acceptance gates.
The earlier retention recovery receipt remains bound to its original source.

`summary.json` records source, provider, binary, scenario and receipt hashes.
