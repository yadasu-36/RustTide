fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/macos.m")
            .file("native/popup.m")
            .flag("-fobjc-arc")
            .flag("-Wno-deprecated-declarations")
            .compile("rusttide_native");
        for framework in [
            "AppKit",
            "ApplicationServices",
            "Security",
            "CFNetwork",
            "NaturalLanguage",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rerun-if-changed=native/macos.m");
        println!("cargo:rerun-if-changed=native/popup.m");
    }
    tauri_build::build();
}
