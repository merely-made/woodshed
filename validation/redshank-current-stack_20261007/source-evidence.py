"""Freeze/verify raw local build inputs and describe exact immutable metadata."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
MERE = "57b4893db6909d5ed9c4ccae30216f0d8164201a"
GENET = "965b64e206a47d1c8808472de9aa461233638768"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(name, value):
    with (HERE / name).open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, ensure_ascii=False)
        stream.write("\n")


def selected_inputs():
    tracked = subprocess.check_output(
        ["git", "ls-files", "ports/redshank", "crates/audio-primitives", "crates/timed-text"],
        cwd=ROOT, text=True, encoding="utf-8",
    ).splitlines()
    paths = [p for p in tracked if Path(p).suffix in {".rs", ".css", ".ttf"}
             or Path(p).name in {"Cargo.toml", "Cargo.lock"}
             or "/tests/fixtures/" in p]
    paths += ["rust-toolchain.toml", "Cargo.toml", "Cargo.lock"]
    paths += [str(p.relative_to(ROOT)).replace("\\", "/")
              for p in HERE.iterdir() if p.suffix in {".py", ".ps1"}]
    return sorted(set(paths))


def freeze():
    rows = []
    with zipfile.ZipFile(HERE / "source-inputs.zip", "x", zipfile.ZIP_DEFLATED) as archive:
        for path in selected_inputs():
            data = (ROOT / path).read_bytes()
            archive.writestr(path, data)
            rows.append({"path": path, "size": len(data), "sha256": digest(data)})
    save("source-manifest.json", {
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "mere": MERE, "genet": GENET,
        "scope": "All tracked port/path-library Rust, Cargo, CSS, bundled fonts and test fixtures; explicit toolchain, parent Cargo files and receipt helpers. Transitive sources are immutable metadata identities.",
        "archive_sha256": digest((HERE / "source-inputs.zip").read_bytes()),
        "inputs": rows,
    })
    verify()


def verify():
    manifest = json.loads((HERE / "source-manifest.json").read_text(encoding="utf-8"))
    assert digest((HERE / "source-inputs.zip").read_bytes()) == manifest["archive_sha256"]
    assert selected_inputs() == [row["path"] for row in manifest["inputs"]]
    with zipfile.ZipFile(HERE / "source-inputs.zip") as archive:
        assert sorted(archive.namelist()) == [row["path"] for row in manifest["inputs"]]
        for row in manifest["inputs"]:
            data = archive.read(row["path"])
            assert len(data) == row["size"] and digest(data) == row["sha256"]
            assert (ROOT / row["path"]).read_bytes() == data, row["path"]
    print(f"Verified {len(manifest['inputs'])} raw source inputs and source archive")


def graph():
    summaries = {}
    for platform in ["windows", "web"]:
        metadata = json.loads((HERE / f"metadata-{platform}.stdout.log").read_text(encoding="utf-8"))
        nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
        packages = {p["id"]: p for p in metadata["packages"]}
        selected = [packages[i] for i in nodes]
        families = {}
        for name, sha in [("mere", MERE), ("genet", GENET)]:
            rows = [p for p in selected if (p["source"] or "").startswith(f"git+https://github.com/merely-made/{name}.git")]
            identities = sorted({p["source"] for p in rows})
            assert len(identities) == 1 and identities[0].endswith("#" + sha), identities
            families[name] = {"source": identities[0], "packages": sorted(p["name"] for p in rows)}
        ipc = [p for p in selected if p["name"] == "ipc-channel"]
        assert len(ipc) == 1 and ipc[0]["source"].endswith("#" + GENET)
        features = nodes[ipc[0]["id"]]["features"]
        assert "force-inprocess" in features and "native-os-ipc" not in features, features
        fonts = [p for p in selected if p["name"] == "fontsan"]
        assert len(fonts) == (1 if platform == "windows" else 0)
        font_features = nodes[fonts[0]["id"]]["features"] if fonts else []
        if fonts:
            assert "wuff" in font_features and "libz-sys" in font_features and "woff2" not in font_features
        assert not any(p["name"] == "fontsan-woff2" for p in selected)
        wgpu = [p for p in selected if p["name"] == "wgpu"]
        assert len(wgpu) == 1 and wgpu[0]["version"] == "30.0.1"
        summaries[platform] = {"selected_packages": len(selected), "families": families,
                               "ipc_features": features, "fontsan_features": font_features,
                               "wgpu": wgpu[0]["version"]}
    save("selected-source-family.json", summaries)
    before = tomllib.loads(subprocess.check_output(["git", "show", "HEAD:ports/redshank/Cargo.lock"], cwd=ROOT, text=True, encoding="utf-8"))
    after = tomllib.loads((ROOT / "ports/redshank/Cargo.lock").read_text(encoding="utf-8"))
    def norm(package):
        package = dict(package)
        source = package.get("source", "")
        if "git+https://github.com/merely-made/" in source:
            source = source.split("?", 1)[0]
            package["source"] = source
        return package
    def table(lock):
        return {(p["name"], p["version"], norm(p).get("source", "")): norm(p) for p in lock["package"]}
    old, new = table(before), table(after)
    save("lock-comparison.json", {
        "before_packages": len(old), "after_packages": len(new),
        "removed": [old[k] for k in sorted(old.keys() - new.keys())],
        "added": [new[k] for k in sorted(new.keys() - old.keys())],
        "changed": [{"before": old[k], "after": new[k]} for k in sorted(old.keys() & new.keys()) if old[k] != new[k]],
        "normalization": "Only Mere/Genet Git source revision queries/fragments normalized for content comparison. Dependency source IDs retained in dependency strings.",
    })
    print(json.dumps({k: {"mere": len(v["families"]["mere"]["packages"]), "genet": len(v["families"]["genet"]["packages"]), "wgpu": v["wgpu"]} for k, v in summaries.items()}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["freeze", "verify", "graph"])
    action = parser.parse_args().action
    {"freeze": freeze, "verify": verify, "graph": graph}[action]()
