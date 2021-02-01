extern crate protobuf_codegen_pure;

fn main() {
    let proto_files = ["protos/config.proto"];
    for f in proto_files.iter() {
        println!("cargo:rerun-if-changed={}", f);
    }

    protobuf_codegen_pure::Codegen::new()
        .out_dir("src/protos/v2ray")
        .inputs(&proto_files)
        .include("protos")
        .run()
        .expect("protoc");
}
