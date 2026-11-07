//! Sign the block-guest component into the pack a host reviews.
//!
//! `build_block_pack --component FILE.wasm --seed-file FILE --out PACK.json` where the seed
//! file holds the 32-byte author seed as 64 hex digits. The author's key is the author's to
//! hold; this tool never generates or stores one. Build the component first:
//! `cargo build -p woodshed-notation --target wasm32-wasip2 --profile block-guest --features block-guest`.
//! The pack asks for `power:block-adapter` and nothing else.

use eidetic::pack::{PackManifest, PackPart, PackPartRole, sign_pack};
use eidetic::{ManifestId, TrustEnvelope};
use personae::{IdentityProvider, InMemoryProvider};

fn arg(name: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| panic!("usage: build_block_pack --component F --seed-file F --out F (missing {name})"))
}

fn main() {
    let component = std::fs::read(arg("--component")).expect("read the component");
    let hex = std::fs::read_to_string(arg("--seed-file")).expect("read the seed file");
    let hex = hex.trim();
    assert_eq!(hex.len(), 64, "the seed is 32 bytes as 64 hex digits");
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex seed");
    }
    let author = InMemoryProvider::from_seed(seed);
    let manifest = PackManifest {
        name: "woodshed-blocks".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        author: author
            .master_public_key()
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        requested_scopes: vec!["power:block-adapter".into()],
        parts: vec![PackPart {
            name: "woodshed-blocks.wasm".into(),
            role: PackPartRole::WasmComponent,
            blob: ManifestId::of_blob(&component),
            bytes: component.len() as u64,
        }],
    };
    let mut trust = TrustEnvelope::self_asserted();
    trust.signatures.push(sign_pack(&manifest, author.master_keypair()));
    let pack = serde_json::json!({ "manifest": manifest, "trust": trust });
    let bytes = serde_json::to_vec(&pack).expect("encode the pack");
    std::fs::write(arg("--out"), &bytes).expect("write the pack");
    println!(
        "pack {} bytes, sha256-blob {}",
        bytes.len(),
        ManifestId::of_blob(&bytes).0.to_hex()
    );
}
