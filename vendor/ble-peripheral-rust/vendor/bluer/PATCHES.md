# BlueR notification-security patch

Upstream: BlueZ/BlueR 0.17.4, imported from the Cargo registry distribution.
Original license and source notices are retained. `.cargo_vcs_info.json`
identifies the upstream revision. This is the single BlueR instance used by
our peripheral supplier, selected through its Linux-only path dependency.

Only gatt/mod.rs and gatt/local.rs behavior changes: expose encrypt-notify and
encrypt-indicate flags and pass the corresponding CharacteristicNotify fields
through to BlueZ. Without these fields the supplier would silently discard
notification encryption requirements. Native BlueZ must accept these flags;
registration errors are propagated and never trigger a security downgrade.
No second D-Bus/GATT server implementation is added.
