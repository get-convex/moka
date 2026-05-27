fn main() {
    if std::env::var("CARGO_FEATURE_SHUTTLE_TESTING").is_ok() {
        println!("cargo:rustc-cfg=moka_shuttle");
    }
}
