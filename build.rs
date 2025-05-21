use std::env;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let lib_name = "tcmalloc_fat"; // Name of the library without 'lib' prefix or '.a' suffix

    println!("cargo:rustc-link-search=native={}", manifest_dir);
    println!("cargo:rustc-link-lib=static={}", lib_name);

    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    }

    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");

    println!("cargo:rerun-if-changed=build.rs");
}
