// Embeds the knob icons and version info into the exe: resource 1 is the
// normal icon (also the exe's own), 2 the grey one shown in the tray while the
// keyboard isn't connected.
fn main() {
    const ICON: &str = "../knob-settings/icons/icon.ico";
    const ICON_DISCONNECTED: &str = "../knob-settings/icons/icon-disconnected.ico";
    // Rebuild when an icon is redrawn.
    println!("cargo:rerun-if-changed={ICON}");
    println!("cargo:rerun-if-changed={ICON_DISCONNECTED}");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ICON)
            .set_icon_with_id(ICON_DISCONNECTED, "2")
            .set("FileDescription", "AK820 Custom Knob")
            .set("ProductName", "AK820 Custom Knob");
        res.compile().expect("cannot embed the icon");
    }
}
