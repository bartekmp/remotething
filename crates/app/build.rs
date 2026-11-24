//! On Windows, puts the app's icon and version information into the `.exe`,
//! for Explorer, the taskbar and the file's Properties dialog.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=packaging/remotething.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        let file = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("remotething.rc");
        std::fs::write(&file, resource_script()).unwrap();
        embed_resource::compile(&file, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}

fn package(name: &str) -> String {
    std::env::var(format!("CARGO_PKG_{name}")).unwrap()
}

/// The resource script: icon 1 is the one Explorer shows.
fn resource_script() -> String {
    let icon = PathBuf::from(package("MANIFEST_DIR")).join("packaging/remotething.ico");
    let icon = icon.display().to_string().replace('\\', "/");
    let (major, minor, patch) = (
        package("VERSION_MAJOR"),
        package("VERSION_MINOR"),
        package("VERSION_PATCH"),
    );
    let version = package("VERSION");
    let author = package("AUTHORS");
    let description = package("DESCRIPTION");
    let numbers = format!("{major},{minor},{patch},0");

    format!(
        r#"1 ICON "{icon}"

1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "{author}"
      VALUE "FileDescription", "{description}"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "remotething"
      VALUE "LegalCopyright", "Copyright (C) 2026 {author}. GNU GPL, version 3."
      VALUE "OriginalFilename", "remotething.exe"
      VALUE "ProductName", "Remotething"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}
