//! The real Woodshed block guest, signed into a pack, reviewed, and loaded through
//! `block-adapter`'s host over the `block-guest` wire: the guest above its host, with no link
//! to an app. This is the one test that proves what the three `Document`-based adapter tests
//! used to prove across the component boundary: a returned attribute map changes only that
//! block's source attribute. (One undo restoring a document byte for byte, and each view
//! building a paintable leaf, are checked in Mere's app-side test.)
#![cfg(feature = "block-guest")]

use block_adapter::{BlockAdapter, BlockInput, GuestAdapter, GuestEngine, RunResult, ViewAsk};
use eidetic::pack::{PackManifest, PackPart, PackPartRole, PackVerdict, sign_pack, verify_pack};
use eidetic::{ManifestId, TrustEnvelope};
use personae::{IdentityProvider, InMemoryProvider};
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::OnceLock;

/// The power a block adapter is granted.
const BLOCK_ADAPTER: &str = "power:block-adapter";

fn component() -> &'static Vec<u8> {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = root.join("target/conformance-wasm");
        let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
        cmd.current_dir(&root).args([
            "build", "--locked", "-p", "woodshed-notation", "--target", "wasm32-wasip2",
            "--profile", "block-guest", "--features", "block-guest", "--target-dir",
        ]).arg(&target);
        for (key, _) in std::env::vars() {
            if key.starts_with("CARGO_") && key != "CARGO_HOME" && key != "CARGO_BUILD_JOBS" {
                cmd.env_remove(key);
            }
        }
        assert!(cmd.status().expect("run cargo").success(), "the guest builds");
        std::fs::read(target.join("wasm32-wasip2/block-guest/woodshed_notation.wasm"))
            .expect("the built component")
    })
}

/// Review as the host does: the signature verifies, the pack inventories exactly this
/// component, it asks for the block-adapter power alone; then the reviewer grants it.
fn reviewed(bytes: &[u8]) -> Vec<String> {
    let author = InMemoryProvider::from_seed([42; 32]);
    let manifest = PackManifest {
        name: "woodshed-blocks".into(),
        version: "test".into(),
        author: author.master_public_key().to_bytes().iter().map(|b| format!("{b:02x}")).collect(),
        requested_scopes: vec![BLOCK_ADAPTER.into()],
        parts: vec![PackPart {
            name: "woodshed-blocks.wasm".into(),
            role: PackPartRole::WasmComponent,
            blob: ManifestId::of_blob(bytes),
            bytes: bytes.len() as u64,
        }],
    };
    let mut trust = TrustEnvelope::self_asserted();
    trust.signatures.push(sign_pack(&manifest, author.master_keypair()));
    assert_eq!(verify_pack(&manifest, &trust), PackVerdict::Trusted, "the pack is signed");
    assert_eq!(manifest.parts.len(), 1);
    assert_eq!(manifest.parts[0].blob, ManifestId::of_blob(bytes), "the pack names these bytes");
    assert_eq!(manifest.requested_scopes, [BLOCK_ADAPTER], "a block adapter asks for no other power");
    manifest.requested_scopes
}

fn attach(engine: &GuestEngine, bytes: &[u8]) -> GuestAdapter {
    GuestAdapter::attach(engine, bytes, &reviewed(bytes)).unwrap()
}

/// The guest's own example, as the block a pane would hold.
fn example_block(adapter: &GuestAdapter, selected: Vec<u32>, pattern: &str) -> (BlockInput, BTreeMap<String, String>) {
    let example = adapter.claim().example.clone().expect("the guest offers an example");
    let mut attrs = example.attrs.clone();
    attrs.insert("id".into(), "progression-test".into());
    let input = BlockInput {
        class: "progression".into(),
        body: example.body,
        attrs: attrs.clone(),
        revision: "r1".into(),
        selected,
        pattern: pattern.into(),
    };
    (input, attrs)
}

#[test]
fn the_real_pack_is_reviewed_and_claims_the_progression_class() {
    let engine = GuestEngine::new().unwrap();
    let bytes = component();
    let adapter = attach(&engine, bytes);
    let claim = adapter.claim();
    assert_eq!(claim.class, "progression");
    assert_eq!(claim.source_key.as_deref(), Some("woodshed"));
    assert_eq!(claim.pattern.as_ref().unwrap().default, "2 + 2");
    let ids: Vec<_> = claim.commands.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["group-bars", "write-notation", "export-plaintext", "backdrop-scene"]);
    assert_eq!(claim.views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["chart", "tab", "score"]);
    // Mere's size cap for a component is 16 MiB.
    assert!(bytes.len() < 16 * 1024 * 1024, "{} bytes", bytes.len());
}

#[test]
fn grouping_changes_only_the_source_attribute() {
    let engine = GuestEngine::new().unwrap();
    let bytes = component();
    let adapter = attach(&engine, bytes);
    let (block, before) = example_block(&adapter, Vec::new(), "2 + 2");
    let rows = adapter.rows(&block).unwrap();
    assert!(rows.len() >= 2, "the example has bars");
    let changes = adapter.choices(&block, "group-bars").unwrap();
    assert!(!changes.is_empty());
    let attrs = adapter.apply(&block, "group-bars", &changes).unwrap();
    assert!(
        attrs.keys().all(|k| before.contains_key(k)) && attrs.len() == before.len(),
        "the map has the same keys"
    );
    let differing: Vec<_> = attrs.iter().filter(|(k, v)| before.get(*k) != Some(*v)).map(|(k, _)| k.as_str()).collect();
    assert_eq!(differing, ["woodshed"], "only the source attribute changes");
}

#[test]
fn write_notation_is_atomic_and_the_three_readings_draw_with_event_regions() {
    let engine = GuestEngine::new().unwrap();
    let bytes = component();
    let adapter = attach(&engine, bytes);
    let (block, _) = example_block(&adapter, Vec::new(), "");
    let RunResult::Attrs(written) = adapter.run(&block, "write-notation", "96").unwrap() else {
        panic!("write-notation returns the whole attribute map")
    };
    assert!(written.contains_key("woodshed"));
    let mut written_block = block.clone();
    written_block.attrs = written;
    for mode in ["chart", "tab", "score"] {
        let view = adapter
            .view(&written_block, &ViewAsk { mode: mode.into(), width: 640.0, selected: Some((0, 0)) })
            .unwrap_or_else(|e| panic!("{mode}: {e}"))
            .expect("the guest has a view");
        let regions = view.regions();
        assert!(!regions.is_empty(), "{mode} names its event regions");
        for r in &regions {
            assert!(r.w > 0.0 && r.h > 0.0 && r.x >= 0.0 && r.x + r.w <= view.width + 0.5, "{mode} region inside");
            assert!(r.label.starts_with("Bar "), "{}", r.label);
        }
        assert!(view.sheet.width > 0.0 && view.sheet.height > 0.0, "{mode} has a sheet");
    }
    // A block with no written notation refuses with the guest's own words, not a trap.
    assert!(adapter
        .view(&block, &ViewAsk { mode: "chart".into(), width: 640.0, selected: None })
        .is_err());
}
