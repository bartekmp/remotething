//! On Windows, puts the app's icon into the `.exe` for Explorer and the taskbar.

fn main() {
    println!("cargo:rerun-if-changed=packaging/remotething.rc");
    println!("cargo:rerun-if-changed=packaging/remotething.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        embed_resource::compile("packaging/remotething.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
