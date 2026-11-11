//! Windows only: embeds the app icon (resource id 1, which GPUI's window class loads) and the
//! version block into `aurum.exe`. Nothing happens for other targets; the guard reads the
//! target OS because build scripts run on the host.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/aurum.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/icon/aurum.ico", "1");
        res.set("ProductName", "Aurum");
        res.set("FileDescription", "Aurum music player");
        res.set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            println!("cargo:warning=cannot embed Windows resources: {e}");
        }
    }
}
