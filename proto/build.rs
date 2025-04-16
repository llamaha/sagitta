use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Tell cargo to rebuild if any proto files change
    println!("cargo:rerun-if-changed=src/proto/vectordb.proto");
    
    // Generate code with file descriptor set for reflection
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("vectordb_descriptor.bin"))
        .out_dir("src/generated")
        .compile(&["src/proto/vectordb.proto"], &["src/proto"])?;
    
    Ok(())
} 