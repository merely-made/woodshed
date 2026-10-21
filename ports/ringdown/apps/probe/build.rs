// SPDX-License-Identifier: MPL-2.0
fn main() {
    println!("cargo:rerun-if-changed=Info.plist");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let plist = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("Info.plist");
        println!(
            "cargo:rustc-link-arg-bin=ringdown-probe=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}
