use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};


const WRAPPER: &str = "netmap.h";


fn main() {

    // Tell cargo to look for shared libraries in the specified directory.
    println!("cargo:rustc-link-search=/usr/local/lib/");

    // Tell cargo to tell rustc to link the system shared libraries.
    println!("cargo:rustc-link-lib=netmap");

    // Tell cargo to invalidate the built crate whenever the wrapper changes.
    println!("cargo:rerun-if-changed={WRAPPER}");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(WRAPPER)
        // Automatically derive useful traits whenever possible
        .derive_debug(true)
        .derive_default(true)
        .derive_eq(true)
        .derive_ord(true)
        .derive_partialeq(true)
        .derive_partialord(true)
        // Generate wrappers for static functions
        .wrap_static_fns(true)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Allow only netmap headers to be included in the binding
        .allowlist_file("[^\\s]*netmap[^\\s]*")
		// Generate documentation comments for bindings
		.generate_comments(true)
        .clang_args(["-fretain-comments-from-system-headers", "-fparse-all-comments"])
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    compile_extern_source_code();

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}


/// Compile the C source code generated by the `wrap_static_fns` option
/// for the bindgen crate.
/// This is required to generate the bindings of static C functions.
fn compile_extern_source_code() {
    let output_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // This is the path to the object file.
    let obj_path = output_path.join("extern.o");
    // This is the path to the static library file.
    let lib_path = output_path.join("libextern.a");

    // Copy header file to folder
    fs::copy(WRAPPER, std::env::temp_dir().join("bindgen").join(WRAPPER))
        .expect("Failed to copy header file");

    // Compile the generated wrappers into an object file.
    let clang_output = std::process::Command::new("clang")
        .arg("-O")
        .arg("-c")
        .arg("-o")
        .arg(&obj_path)
        .arg(std::env::temp_dir().join("bindgen").join("extern.c"))
        .arg("-include")
        .arg(WRAPPER)
        .output()
        .unwrap();

    if !clang_output.status.success() {
        panic!(
            "Could not compile object file:\n{}",
            String::from_utf8_lossy(&clang_output.stderr)
        );
    }

    // Turn the object file into a static library
    let lib_output = Command::new("ar")
        .arg("rcs")
        .arg(lib_path)
        .arg(obj_path)
        .output()
        .unwrap();

    if !lib_output.status.success() {
        panic!("Could not emit library file:\n");
    }

    // Tell cargo to statically link against the `libextern` static library.
    println!("cargo:rustc-link-search={}", env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-lib=static=extern");
}
