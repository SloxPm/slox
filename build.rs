fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=crates/slox-cli");
    println!("cargo:rerun-if-changed=crates/slox-core");
    println!("cargo:rerun-if-changed=crates/slox-bootstrap");
    println!("cargo:rustc-env=SLOX_WORKSPACE_LAYOUT=crates");
}
