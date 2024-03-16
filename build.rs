use std::env;
use std::fs;
use std::path::Path;

// Example custom build script.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir_string = env::var("CARGO_MANIFEST_DIR").unwrap();
    let build_type = env::var("PROFILE").unwrap();
    let path = Path::new(&manifest_dir_string).join("target").join(build_type);

    let dest_path = path.join("steam_appid.txt");

    fs::write(
        &dest_path,
        "1384160"
    ).unwrap();
}