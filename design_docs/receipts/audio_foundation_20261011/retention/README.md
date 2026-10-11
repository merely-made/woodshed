# Redshank bounded generation acceptance

Source `9632f37`; binary and scenario hashes are in `summary.json`. The native
launcher copied the shipping desktop binary into three finite LaunchServices
processes. Each produced `RESULT ok`, one nonblank capture and a successful
process receipt. All three images were reviewed at full frame.

An isolated empty listener began with twenty published generations. Eleven
actual settings writes retained generations30,31,32 (Light,HC Dark,HC Light),
while an interrupted pending generation21 and unrelated media bytes survived.
A fresh process restoredHC Light. After deliberately corrupting generations31
and32, another fresh process recovered generation30/Light. The saved published
states are preserved here before that corruption.

The rejected script receipt remains: its unsupported `await-idle` verb caused
a failed result. The corrected scripts use the documented `wait300` command
(spelled `wait 300` in source), with a newly seeded isolated profile.

This establishes desktop publication/retention and fresh-process recovery.
It does not qualify physical playback, microphone timing, or the later
Firewheel consumer pin. Existing empty-player icon rendering is outside this
storage slice.
