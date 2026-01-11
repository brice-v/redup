fn main() {
    println!("cargo:rustc-env=REDUP_VERSION={}", env!("CARGO_PKG_VERSION"));
}
