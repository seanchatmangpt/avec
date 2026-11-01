//! Build script for cgap-beam.
//!
//! The NIF dylib leaves every `enif_*` symbol undefined at link time; the BEAM
//! resolves them when `erlang:load_nif/2` dlopens the library. On macOS
//! (aarch64 included) rustc no longer passes `-undefined dynamic_lookup`
//! automatically, so supply it here.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
    }
}
