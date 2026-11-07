//! The real Woodshed block guest, signed into a pack, reviewed, and loaded through Mere's
//! host: the guest above its host. This is the one test that proves what the three
//! `Document`-based adapter tests used to prove (a returned attribute map changes only that
//! block, in one undo step) across the component boundary.
#![cfg(feature = "block-guest")]

use eidetic::pack::{PackManifest, PackPart, PackPartRole, sign_pack};
use eidetic::{ManifestId, TrustEnvelope};
use graphshell::block_editing::adapter::{BlockAdapter, BlockInput, RunResult, ViewAsk};
use graphshell::block_editing::sheet_leaf::{ColorF, SheetLeaf, SheetTheme};
use graphshell::block_editing::{Document, Preview, wrap};
use graphshell::capsule_applet::{BLOCK_ADAPTER, CapsuleDisclosure, CapsuleMount, SignedPack};
use graphshell_blocks::{GuestAdapter, GuestEngine};
use personae::{IdentityProvider, InMemoryProvider};
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::OnceLock;

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

fn hash(bytes: &[u8]) -> String {
    ManifestId::of_blob(bytes).0.to_hex()
}

fn reviewed(bytes: &[u8]) -> CapsuleMount {
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
    let pack = serde_json::to_vec(&SignedPack { manifest, trust }).unwrap();
    let disclosure = CapsuleDisclosure {
        moot: "moot:test".into(),
        collection: "collection:block-adapters".into(),
        revision: "frontier".into(),
        capsules: vec![],
    };
    let mut mount = CapsuleMount::review(&pack, &hash(&pack), bytes, disclosure).unwrap();
    mount.approve(vec![BLOCK_ADAPTER.into()]).unwrap();
    mount
}

fn document_with_example(adapter: &GuestAdapter) -> (Document, BTreeMap<String, String>) {
    let example = adapter.claim().example.clone().expect("the guest offers an example");
    let mut attrs = example.attrs.clone();
    attrs.insert("id".into(), "progression-test".into());
    let source = format!("# Chart\n\n{}", wrap(&attrs, &example.body));
    (Document::new("conformance", &source), attrs)
}

fn input(doc: &Document, selected: Vec<u32>, pattern: &str) -> (graphshell::block_editing::Div, BlockInput) {
    let div = doc.blocks().into_iter().next().expect("one block");
    let source = doc.editor.source();
    let input = BlockInput {
        class: "progression".into(),
        body: source[div.body.clone()].to_owned(),
        attrs: div.attributes.clone(),
        revision: "r1".into(),
        selected,
        pattern: pattern.into(),
    };
    (div, input)
}

#[test]
fn the_real_pack_is_reviewed_and_claims_the_progression_class() {
    let engine = GuestEngine::new().unwrap();
    let bytes = component();
    let adapter = GuestAdapter::attach(&engine, &reviewed(bytes), bytes).unwrap();
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
fn grouping_changes_only_that_block_in_one_undo_step() {
    let engine = GuestEngine::new().unwrap();
    let bytes = component();
    let adapter = GuestAdapter::attach(&engine, &reviewed(bytes), bytes).unwrap();
    let (mut doc, _) = document_with_example(&adapter);
    let before = doc.editor.source().to_owned();
    let (div, block) = input(&doc, Vec::new(), "2 + 2");
    let rows = adapter.rows(&block).unwrap();
    assert!(rows.len() >= 2, "the example has bars");
    let changes = adapter.choices(&block, "group-bars").unwrap();
    assert!(!changes.is_empty());
    let attrs = adapter.apply(&block, "group-bars", &changes).unwrap();
    assert!(
        attrs.keys().all(|k| div.attributes.contains_key(k)) && attrs.len() == div.attributes.len(),
        "the map has the same keys"
    );
    let differing: Vec<_> = attrs.iter().filter(|(k, v)| div.attributes.get(*k) != Some(*v)).map(|(k, _)| k.as_str()).collect();
    assert_eq!(differing, ["woodshed"], "only the source attribute changes");
    let preview = Preview::new(doc.editor.source(), div, changes).unwrap();
    doc.apply(&preview, attrs).unwrap();
    assert_ne!(doc.editor.source(), before);
    assert!(doc.editor.source().starts_with("# Chart\n\n"), "text outside the block is untouched");
    doc.editor.apply(cambium::TextCommand::Undo);
    assert_eq!(doc.editor.source(), before, "one undo restores the document byte for byte");
}

#[test]
fn write_notation_is_atomic_and_the_three_readings_draw_with_event_regions() {
    let engine = GuestEngine::new().unwrap();
    let bytes = component();
    let adapter = GuestAdapter::attach(&engine, &reviewed(bytes), bytes).unwrap();
    let (doc, _) = document_with_example(&adapter);
    let (_, block) = input(&doc, Vec::new(), "");
    let RunResult::Attrs(written) = adapter.run(&block, "write-notation", "96").unwrap() else {
        panic!("write-notation returns the whole attribute map")
    };
    assert!(written.contains_key("woodshed"));
    let mut written_block = block.clone();
    written_block.attrs = written;
    let theme = SheetTheme {
        ink: ColorF { r: 0.1, g: 0.1, b: 0.1, a: 1.0 },
        accent: ColorF { r: 0.1, g: 0.4, b: 0.9, a: 1.0 },
        muted: ColorF { r: 0.5, g: 0.5, b: 0.5, a: 1.0 },
    };
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
        assert!(SheetLeaf::new(view.sheet.clone(), theme).is_ok(), "{mode} paints");
    }
    // A block with no written notation refuses with the guest's own words, not a trap.
    assert!(adapter
        .view(&block, &ViewAsk { mode: "chart".into(), width: 640.0, selected: None })
        .is_err());
}
