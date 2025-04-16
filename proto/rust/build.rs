use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Tell cargo to rebuild if the proto file changes
    println!("cargo:rerun-if-changed=src/proto/vectordb.proto");
    
    // Configure tonic-build
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("vectordb_descriptor.bin"))
        .compile(&["src/proto/vectordb.proto"], &["src/proto"])?;
    
    Ok(())
} 