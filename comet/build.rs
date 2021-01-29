extern crate protobuf_codegen_pure;

fn main() {
    protobuf_codegen_pure::Codegen::new()
        .out_dir("src/protos/v2ray")
        .inputs(&["protos/config.proto", "protos/network.proto", "protos/port.proto"])
        .include("protos")
        .run()
        .expect("protoc");
}
