// Embeds the knob icon and version info into the exe.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../knob-settings/icons/icon.ico")
            .set("FileDescription", "AK820 Custom Knob")
            .set("ProductName", "AK820 Custom Knob");
        res.compile().expect("cannot embed the icon");
    }
}
