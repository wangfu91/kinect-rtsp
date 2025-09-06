use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Tell cargo to rerun this script if the launcher script changes
    println!("cargo:rerun-if-changed=run.ps1");

    // Determine output directory (target/{debug|release})
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    // OUT_DIR is something like target/debug/build/<pkg>/out
    // Walk up to target/{profile}
    let mut path = PathBuf::from(out_dir);
    // pop "out", "<pkg>", "build"
    for _ in 0..3 {
        path.pop();
    }
    // Now path should be target/{debug|release}

    // Copy run.ps1 (friendly name) next to the built binary
    let dest = path.join("run.ps1");
    let src = PathBuf::from("run.ps1");

    if let Err(e) = fs::copy(&src, &dest) {
        // Build script should not fail the build for copy errors; just warn.
        eprintln!(
            "cargo:warning=Failed to copy {} to {}: {}",
            src.display(),
            dest.display(),
            e
        );
    }
}
