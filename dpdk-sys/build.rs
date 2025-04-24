//use std::env;
//use std::path::PathBuf;
//
//fn main() {
//    pkg_config::Config::new()
//        .probe("libdpdk")
//        .expect("Could not find dpdk via pkg-config");
//
//    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
//    let bindings = bindgen::Builder::default()
//        .header("lib.h")
//        .blocklist_type("rte_ether_addr")
//        .blocklist_type("rte_arp_ipv4")
//        .blocklist_type("rte_arp_hdr")
//        .blocklist_type("rte_l2tpv2_combined_msg_hdr")
//        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
//
//        .generate_inline_functions(true)
//        //.wrap_static_fns(true)
//        .wrap_static_fns_path(out_path.clone().join("extern"))
//
//        .generate()
//        .expect("Unable to generate bindings for DPDK");
//
//    bindings
//        .write_to_file(out_path.join("bindings.rs"))
//        .expect("Couldn't write bindings!");
//}


use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=lib.c");
    println!("cargo:rerun-if-changed=lib.h");
    // Use pkg-config to locate the dpdk library and get its compile options.
    let dpdk = pkg_config::Config::new()
        //.statik(true)
        .probe("libdpdk")
        .expect("Could not find dpdk via pkg-config");

    // Create a new cc::Build instance for compiling lib.c.
    let mut build = cc::Build::new();
    build.file("lib.c");

    // Pass include paths from pkg-config to the cc build.
    for include_path in dpdk.include_paths {
        build.include(include_path);
    }

    // If pkg-config provided any compile definitions, pass them as well.
    for (define, value) in dpdk.defines {
        build.define(&define, value.as_ref().map(|s| s.as_str()));
    }

    println!("cargo:rustc-link-lib=rte_bus_vdev");

    build.flag("-mssse3");
    build.flag("-O3");
    //build.flag("-lrte_bus_vdev");
    // Compile lib.c into a static library (e.g., liblib.a).
    build.compile("lib");

    // Set up the output directory for generated bindings.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Generate Rust bindings from lib.h using bindgen.
    let bindings = bindgen::Builder::default()
        .header("lib.h")
        .blocklist_type("rte_ether_addr")
        .blocklist_type("rte_arp_ipv4")
        .blocklist_type("rte_arp_hdr")
        .blocklist_type("rte_l2tpv2_combined_msg_hdr")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate_inline_functions(true)
        // Optionally, wrap static functions (adjust as needed)
        .wrap_static_fns_path(out_path.join("extern"))
        .generate()
        .expect("Unable to generate bindings for DPDK");

    // Write the generated bindings to $OUT_DIR/bindings.rs.
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
