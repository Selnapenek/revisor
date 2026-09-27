fn main() {
    println!("cargo:rerun-if-env-changed=REVISOR_GITHUB_CLIENT_ID");
    tauri_build::build()
}
