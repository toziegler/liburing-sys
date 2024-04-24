use bindgen;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=c/liburing/");

    let liburing_src = Path::new("./c/liburing/src/");
    let liburing_out = Path::new(&out_dir).join("liburing");

    if !liburing_src.exists() {
        let status = Command::new("git")
            .args(["submodule", "update", "--init", "c/liburing"])
            .status()
            .expect("Failed to update git submodule");
        assert!(status.success(), "Git submodule update failed");
    }

    if !liburing_out.exists() {
        Command::new("cp")
            .arg("-r")
            .arg("c/liburing")
            .arg(&out_dir)
            .status()
            .expect("Failed to copy liburing to OUT_DIR");

        Command::new("make")
            .arg("-j")
            .current_dir(&liburing_out)
            .status()
            .expect("Failed to build liburing.a");
    }

    println!("cargo:rustc-link-lib=static=uring-ffi");
    // println!("cargo:rustc-link-search=native={}/liburing/src", out_dir);
    println!(
        "cargo:rustc-link-search=native={}",
        liburing_out.join("src").display()
    );

    // Generate bindings
    let bindings = bindgen::Builder::default()
        .allowlist_function("__io_uring.*")
        .allowlist_function("io_uring.*")
        .allowlist_var("IORING.*")
        .allowlist_var("IOSQE.*")
        .allowlist_item("IORING.*")
        .allowlist_item("IOSQE.*")
        .allowlist_type("io_uring.*")
        .clang_arg(format!("-I{}/liburing/src/include", out_dir))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .header("c/wrapper.h")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
