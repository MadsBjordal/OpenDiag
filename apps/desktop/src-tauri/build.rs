fn main() {
    // The release workflow sets the GitHub release tag this build belongs to.
    println!("cargo:rerun-if-env-changed=OPENDIAG_RELEASE");
    tauri_build::build()
}
