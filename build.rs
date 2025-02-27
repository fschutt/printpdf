pub fn main() {
    if std::env::var("TARGET")
        .expect("Unable to get TARGET")
        .contains("wasm")
    {
        println!("cargo:rustc-cfg=feature=\"wasm\"");
    }
}
