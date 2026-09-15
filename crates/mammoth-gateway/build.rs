use std::{fs, path::PathBuf};
fn main() {
    let root =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../ui/build");
    let dest = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("ui");
    if dest.exists() {
        fs::remove_dir_all(&dest).unwrap();
    }
    fs::create_dir_all(&dest).unwrap();
    if root.join("index.html").exists() {
        copy(&root, &dest);
    } else {
        fs::write(dest.join("index.html"), "<!doctype html><html><head><title>Mammoth</title></head><body><h1>Mammoth is running</h1><p>Build the dashboard with <code>cargo xtask build-ui</code>, then rebuild Mammoth.</p><p><a href='/api/v1/cluster/report'>Cluster report</a></p></body></html>").unwrap();
    }
    let generated = format!(
        "#[derive(rust_embed::RustEmbed)]\n#[folder = {:?}]\nstruct Assets;\n",
        dest.to_str().unwrap()
    );
    fs::write(dest.parent().unwrap().join("assets.rs"), generated).unwrap();
    println!("cargo:rerun-if-changed={}", root.display());
}
fn copy(from: &std::path::Path, to: &std::path::Path) {
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            fs::create_dir_all(&dst).unwrap();
            copy(&e.path(), &dst);
        } else {
            fs::copy(e.path(), dst).unwrap();
        }
    }
}
