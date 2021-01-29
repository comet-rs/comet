extern crate protoc_rust;

fn main() {
    protoc_rust::Codegen::new()
        .out_dir("src/protos/v2ray")
        .inputs(&["protos/config.proto", "protos/network.proto", "protos/port.proto"])
        .include("protos")
        .run()
        .expect("protoc");
}
