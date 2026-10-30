fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("metal_shim.m")
            .flag("-fobjc-arc")
            .compile("metal_shim");

        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}
