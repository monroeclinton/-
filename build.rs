use std::path::Path;

use libbpf_cargo::SkeletonBuilder;

const SRC: &str = "./src/bpf/socket_redirector.bpf.c";

fn main() {
    std::fs::create_dir_all("./src/bpf/.output").unwrap();
    let skel = Path::new("./src/bpf/.output/socket_redirector.skel.rs");
    SkeletonBuilder::new()
        .source(SRC)
        .build_and_generate(skel)
        .unwrap();
    println!("cargo:rerun-if-changed={}", SRC);
}
