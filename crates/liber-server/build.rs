use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=EMBED_UI");
    println!("cargo:rerun-if-changed=../../frontend/dist/index.html");
    let want_embed = std::env::var("EMBED_UI").as_deref() == Ok("1");
    println!("cargo:warning=EMBED_UI check: want_embed={want_embed}");
    let dist: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../frontend/dist/index.html");
    println!("cargo:warning=dist exists: {}", dist.is_file());
    if want_embed && dist.is_file() {
        println!("cargo:rustc-cfg=embed_frontend");
    }
}
