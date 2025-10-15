use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command};

fn main() {
    // Re-run hints
    println!("cargo:rerun-if-changed=bindings.h");
    println!("cargo:rerun-if-env-changed=CFLAGS");
    println!("cargo:rerun-if-env-changed=NUM_JOBS");
    println!("cargo:rerun-if-env-changed=LIBXDP_SYS_ALLOW_NON_PIC");

    let src_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let xdptools_dir = src_dir.join("xdp-tools");
    let libxdp_dir = xdptools_dir.join("lib/libxdp");
    let headers_dir = xdptools_dir.join("headers/xdp");

    let libbpf_dir = xdptools_dir.join("lib/libbpf/src");
    let bpf_headers_dir = libbpf_dir.join("root/usr/include");

    // Ensure -fPIC (and keep user-provided flags)
    let mut cflags = env::var("CFLAGS").unwrap_or_default();
    if !cflags.split_whitespace().any(|f| f == "-fPIC") {
        if !cflags.is_empty() { cflags.push(' '); }
        cflags.push_str("-fPIC");
    }
    if !cflags.split_whitespace().any(|f| f == "-fvisibility=hidden") {
        cflags.push_str(" -fvisibility=hidden");
    }
    // Push PIC through both knobs; some Makefiles only honor one of them
    let extra_flags = env::var("EXTRA_CFLAGS");
    let extra_cflags = if extra_flags.as_ref().ok().map_or(true, |v| !v.contains("-fPIC")) {
        "-fPIC"
    } else {
        let var = extra_flags.unwrap();
        &var.as_str().to_string()
    };

    let jobs = env::var("NUM_JOBS").unwrap_or_else(|_| "1".into());

    // ---- Clean first to avoid stale non-PIC objects ----
    run(
        Command::new("make")
            .current_dir(&libbpf_dir)
            .arg("clean"),
        "make -C libbpf/src clean",
    );
    run(
        Command::new("make")
            .current_dir(&xdptools_dir)
            .arg("clean"),
        "make -C xdp-tools clean",
    );

    // ---- Rebuild libbpf (static, PIC) forcing rebuild (-B) ----
    run(
        Command::new("make")
            .current_dir(&libbpf_dir)
            .env("BUILD_STATIC_ONLY", "1")
            .env("CFLAGS", &cflags)
            .env("EXTRA_CFLAGS", extra_cflags)
            .arg("-B")                // force rebuild
            .arg("-j").arg(&jobs)
            .arg("libbpf.a")
            .arg("V=1"),
        "make libbpf (static, PIC)",
    );

    // ---- Rebuild libxdp (static, PIC) forcing rebuild (-B) ----
    run(
        Command::new("make")
            .current_dir(&xdptools_dir)
            .env("CFLAGS", &cflags)
            .env("EXTRA_CFLAGS", extra_cflags)
            .arg("-B")                // force rebuild
            .arg("-j").arg(&jobs)
            .arg("libxdp")
            .arg("V=1"),
        "make libxdp (static, PIC)",
    );

    // Paths to produced static archives
    let libbpf_a = libbpf_dir.join("libbpf.a");
    let libxdp_a = libxdp_dir.join("libxdp.a");

    // Assert archives look PIC-safe (fail early)
    if env::var_os("LIBXDP_SYS_ALLOW_NON_PIC").is_none() {
        assert_pic_archive(&libbpf_a, "libbpf.a");
        assert_pic_archive(&libxdp_a, "libxdp.a");
    } else {
        warn("Skipping PIC check (LIBXDP_SYS_ALLOW_NON_PIC is set)");
    }

    // Link vendored static libs
    println!("cargo:rustc-link-search=native={}", libxdp_dir.display());
    println!("cargo:rustc-link-search=native={}", libbpf_dir.display());
    println!("cargo:rustc-link-lib=static=xdp");
    println!("cargo:rustc-link-lib=static=bpf");
    println!("cargo:rustc-link-lib=elf");
    println!("cargo:rustc-link-lib=z");

    // Bindings (unchanged from yours)
    bindgen::Builder::default()
        .header("bindings.h")
        .generate_inline_functions(true)
        .clang_arg(format!("-I{}", bpf_headers_dir.display()))
        .clang_arg(format!("-I{}", headers_dir.display()))
        .allowlist_var("BPF_.*")
        .allowlist_var("LIBBPF.*")
        .allowlist_var("XDP_.*")
        .allowlist_var("MAX_DISPATCHER_ACTIONS")
        .allowlist_var("XSK_.*")
        .allowlist_var("BTF_.*")
        .allowlist_function("xdp_.*")
        .allowlist_function("libxdp_.*")
        .allowlist_function("xsk_.*")
        .allowlist_function("btf_.*")
        .allowlist_function("bpf_.*")
        .allowlist_type("xsk_.*")
        .allowlist_type("xdp_.*")
        .allowlist_type("bpf_.*")
        .allowlist_type("btf_.*")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(src_dir.join("src/bindings.rs"))
        .expect("Couldn't write bindings");
}

fn run(cmd: &mut Command, what: &str) {
    println!("cargo:warning=building: {} -> {:?}", what, cmd);
    let status = cmd.status().expect("could not execute make");
    assert!(status.success(), "{} failed", what);
}

// Inspect the archive for absolute 32-bit relocs that indicate non-PIC objects.
fn assert_pic_archive(archive: &Path, name: &str) {
    if !archive.exists() {
        warn(&format!("{} not found at {}, skipping PIC check", name, archive.display()));
        return;
    }

    let checks = [
        ("objdump", vec!["-r", archive.to_str().unwrap()]),
        ("llvm-objdump", vec!["-r", archive.to_str().unwrap()]),
        ("readelf", vec!["-rW", archive.to_str().unwrap()]),
    ];

    for (tool, args) in checks {
        if which(tool) {
            let out = Command::new(tool)
                .args(&args)
                .output()
                .expect("failed to run objdump/readelf for PIC check");
            let text = String::from_utf8_lossy(&out.stdout);
            if text.contains("R_X86_64_32") || text.contains("R_X86_64_32S") {
                panic!(
                    concat!(
                        "{} contains non-PIC relocations ({}). ",
                        "Rebuild with -fPIC. If this is a false positive, set LIBXDP_SYS_ALLOW_NON_PIC=1 to bypass the check.\n",
                        "First few hits:\n{}"
                    ),
                    name,
                    archive.display(),
                    preview_lines(&text, 12)
                );
            } else {
                println!("cargo:warning=PIC check OK for {}", name);
            }
            return;
        }
    }
    warn("No objdump/llvm-objdump/readelf found; skipping PIC check");
}

fn which(bin: &str) -> bool {
    env::var_os("PATH")
        .and_then(|p| env::split_paths(&p).find(|d| d.join(bin).exists()))
        .is_some()
}

fn warn(msg: &str) {
    println!("cargo:warning={}", msg);
}

fn preview_lines(s: &str, n: usize) -> String {
    s.lines().take(n).collect::<Vec<_>>().join("\n")
}
