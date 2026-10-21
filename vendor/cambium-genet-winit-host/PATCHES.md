# Shared host supplier patch

Source: Mere `7de937d0e708db24d9a325cbff3fbe5beb3a4d3c`,
`crates/cambium/cambium-genet-winit-host`, MPL-2.0. `UPSTREAM.json` pins original
file hashes. The package manifest expands inherited workspace/path dependency
entries to the same original Git identities; no host dependency is reimplemented.

The existing native event owner ignores synthesized **presses** before dispatch.
Winit uses those to describe keys held when focus returns on Windows/X11. They
are not fresh user gestures. Releases retain the original Tab-navigation path.
A pure boundary test checks admission for real vs synthesized events. Product
repeat suppression still uses the shared `KeyPress.repeat` field.

The root source patch resolves this as the **single** Cambium desktop host for
all consumers. There is no second host/event loop or app-local keyboard adapter.
The correction belongs at this same event owner in Mere. Once that owner source
contains it and a dependency update consumes that revision, remove the root
patch and this snapshot together. Do not extend this supplier with app policy.

The October 21 integration rebases this same admission change onto the canonical
Woodshed supplier cohort; its Cambium/Sprigging and AccessKit dependency versions
match that owner revision. Historical device receipts retain their original hashes.
