# Independent workspace and release boundary checks

The CI lane qualifies commits `84ad201` and `73df290`, based on `95abb13`.
`receipt.json` records source/lock hashes, commands,1328 passing CPU checks,
eight ignored custody/device fixtures, four locked metadata results and eight
PowerShell fixture cases. Logs and the packaging reproducer are preserved here.
The frozen wrapper replays all exit zero. Earlier interrupted wrapper logs are
retained; their test totals alone did not qualify the shell wrapper.

The Hocket run deliberately uses the explicit pre-extraction package scope
because `hocket-project` is absent at this lane's base. Integrated project/host
qualification follows in the family receipt. Root host/Looper changes and both
Firewheel consumer pins also have later source-bound gates.

The fixture suite executes the actual release packaging block. Stable and
prerelease Woodshed tags produce correct version/channel outputs; other product,
legacy and malformed tags are refused. It uses fixture executable bytes on
macOS, so it does not qualify Windows binaries or actual release publication.
