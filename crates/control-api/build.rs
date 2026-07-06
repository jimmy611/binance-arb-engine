//! Generates the Rust server stubs from the protobuf contract.
//!
//! The `.proto` files under `proto/` are the single source of truth for the API.
//! We derive serde on every generated message so the same types serialize to
//! JSON over the streaming WebSocket, keeping the wire contract unified.

use std::path::PathBuf;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let proto_root = PathBuf::from(&manifest).join("../../proto");
    let arb = proto_root.join("arbengine/v1/arb.proto");
    let common = proto_root.join("arbengine/v1/common.proto");

    println!("cargo:rerun-if-changed={}", arb.display());
    println!("cargo:rerun-if-changed={}", common.display());

    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&[arb, common], &[proto_root])
        .expect("failed to compile protos");
}
