// build.rs
use std::{env, path::PathBuf};

fn main() {
    // --- 1) Parse env flags --------------------------------------------------
    // We accept typical envs:
    //   CFLAGS   -> -I / -D (compile-time)
    //   LDFLAGS  -> -L / -l (link-time)
    let cflags  = env::var("CFLAGS").unwrap_or_default();
    let ldflags = env::var("LDFLAGS").unwrap_or_default();

    let mut include_dirs: Vec<String> = Vec::new();
    let mut defines: Vec<String> = Vec::new();

    // From CFLAGS: collect -I and -D for both cc and bindgen/clang
    for flag in cflags.split_whitespace() {
        if let Some(path) = flag.strip_prefix("-I") {
            include_dirs.push(path.to_string());
        } else if let Some(def) = flag.strip_prefix("-D") {
            defines.push(def.to_string());
        } else if let Some(path) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(name) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={name}");
        }
    }

    // From LDFLAGS: pass -L/-l to rustc
    for flag in ldflags.split_whitespace() {
        if let Some(path) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(name) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={name}");
        }
    }

    // --- 2) Rebuild triggers --------------------------------------------------
    // Re-run if wrapper or any env that affects codegen changes.
    println!("cargo:rerun-if-changed=include/wrapper.h");
    println!("cargo:rerun-if-env-changed=CFLAGS");
    println!("cargo:rerun-if-env-changed=LDFLAGS");
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");

    println!("cargo:rustc-link-lib=dylib=tux");

    // --- 3) (Optional) compile any bundled C sources -------------------------
    // If you have .c files, add them here; otherwise you can remove this block.
    let mut cc_build = cc::Build::new();
    for dir in &include_dirs {
        cc_build.include(dir);
    }
    for def in &defines {
        // Accept "NAME" or "NAME=VALUE"
        if let Some((k, v)) = def.split_once('=') {
            cc_build.define(k, Some(v));
        } else {
            cc_build.define(def, None);
        }
    }
    // Example:
    // cc_build.file("csrc/mylib.c").compile("mylib");

    // --- 4) Generate bindings with bindgen -----------------------------------
    // Skip bindgen on docs.rs (no libclang). You can also gate with a feature.
    let building_docs = env::var("DOCS_RS").is_ok();
    if !building_docs {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let wrapper = manifest_dir.join("include/wrapper.h");

        let mut builder = bindgen::Builder::default()
            .header(wrapper.to_string_lossy())
            .layout_tests(false)
            .formatter(bindgen::Formatter::Rustfmt);

        // Forward include dirs and defines to clang so <angled> includes resolve.
        for dir in &include_dirs {
            builder = builder.clang_arg(format!("-I{dir}"));
        }
        for def in &defines {
            builder = builder.clang_arg(format!("-D{def}"));
        }

        // (Optional) curate what you pull in:
        // builder = builder
        //     .allowlist_function("my_.*")
        //     .allowlist_type("my_.*")
        //     .allowlist_var("MY_.*");

        let bindings = builder
            .generate()
            .expect("bindgen failed to generate bindings");

        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out.join("bindings.rs"))
            .expect("Couldn't write bindings.rs");
    }
}
