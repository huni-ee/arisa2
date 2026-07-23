use std::{path::PathBuf, process::Command};

const SIGNAL_CHAIN_SYMBOLS: &[&str] = &[
    "SetSpecialSignalHandlerFn",
    "GetSpecialSignalHandlerFn",
    "EnsureFrontOfChain",
    "InitializeSignalChain",
    "AddSpecialSignalHandlerFn",
    "RemoveSpecialSignalHandlerFn",
];

fn main() {
    println!("cargo:rerun-if-env-changed=ARISA_COMMIT_SHA");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/main");
    println!("cargo:rustc-env=ARISA_COMMIT_SHA={}", commit_sha());

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    let protobuf_include = protoc_bin_vendored::include_path().expect("protobuf include path");

    let mut prost_config = tonic_prost_build::Config::new();
    prost_config.protoc_executable(protoc.clone());
    let descriptor_path = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
        .join("arisa_descriptor.bin");

    tonic_prost_build::configure()
        .build_client(false)
        .file_descriptor_set_path(descriptor_path)
        .compile_with_config(
            prost_config,
            &["proto/arisa.proto"],
            &[
                "proto",
                protobuf_include.to_str().expect("UTF-8 include path"),
            ],
        )
        .expect("failed to compile protobuf contract");

    let mut preferences_config = tonic_prost_build::Config::new();
    preferences_config.protoc_executable(protoc);
    preferences_config
        .compile_protos(&["proto/preferences.proto"], &["proto"])
        .expect("failed to compile preferences protobuf");

    println!("cargo:rerun-if-changed=proto/arisa.proto");
    println!("cargo:rerun-if-changed=proto/preferences.proto");

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("android") {
        return;
    }

    for symbol in SIGNAL_CHAIN_SYMBOLS {
        println!("cargo:rustc-link-arg-bins=-Wl,-u,{symbol}");
        println!("cargo:rustc-link-arg-bins=-Wl,--export-dynamic-symbol={symbol}");
    }
}

fn commit_sha() -> String {
    std::env::var("ARISA_COMMIT_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let output = Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}
