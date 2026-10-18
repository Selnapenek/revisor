// Bake REVISOR_GITHUB_CLIENT_ID into the binary. Shell env wins over the repo `.env`.
fn main() {
    println!("cargo:rerun-if-env-changed=REVISOR_GITHUB_CLIENT_ID");
    println!("cargo:rerun-if-changed=../../.env");
    if std::env::var("REVISOR_GITHUB_CLIENT_ID").is_ok() {
        return;
    }
    let Ok(env) = std::fs::read_to_string("../../.env") else { return };
    for line in env.lines() {
        if let Some(value) = line.trim().strip_prefix("REVISOR_GITHUB_CLIENT_ID=") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            println!("cargo:rustc-env=REVISOR_GITHUB_CLIENT_ID={value}");
        }
    }
}
