// The web UI (web/build) is embedded into the binary. Make sure the folder
// exists so the daemon also builds before `npm run build` has ever run.
fn main() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/build");
    std::fs::create_dir_all(&ui).expect("creating web/build");
    println!("cargo:rerun-if-changed=../../web/build");
}
