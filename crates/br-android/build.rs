fn main() {
    slint_build::compile("ui/app.slint").unwrap();
    if std::env::var("TARGET").is_ok_and(|t| t.contains("android")) {
        println!("cargo:rustc-cdylib-link-arg=-lc++_static");
    }
}
