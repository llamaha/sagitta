use std::env;
use std::fs;
use std::path::{PathBuf};
// use fs_extra::dir as fs_dir; // Removed unused import
use fs_extra::file as fs_file;
// use std::path::Path; // Removed unused import
// use std::process::Command; // Removed if not needed elsewhere

// Function to find the directory containing ONNX Runtime libraries in the cache
fn find_onnx_runtime_lib_dir() -> Option<PathBuf> {
    let home_dir = dirs::home_dir()?;
    let cache_base = home_dir.join(".cache/ort.pyke.io/dfbin");

    let target_triple = env::var("TARGET").ok()?;
    let target_cache_dir = cache_base.join(&target_triple);

    if !target_cache_dir.is_dir() {
        println!("cargo:warning=build.rs: ONNX Runtime cache directory not found for target {} at {}", target_triple, target_cache_dir.display());
        return None;
    }

    // Search for a subdirectory within the target cache dir (likely a hash)
    for entry in fs::read_dir(&target_cache_dir).ok()?.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            let lib_dir = path.join("onnxruntime/lib");
            if lib_dir.is_dir() {
                // Basic check: does it contain *any* .so or .dylib file?
                let has_libs = fs::read_dir(&lib_dir).ok()?.any(|f| {
                    f.map_or(false, |e| {
                        let p = e.path();
                        p.is_file() && (p.extension().map_or(false, |ext| ext == "so" || ext == "dylib"))
                    })
                });
                if has_libs {
                    println!("cargo:warning=build.rs: Found potential ONNX Runtime library directory: {}", lib_dir.display());
                    return Some(lib_dir);
                }
            }
        }
    }

    println!("cargo:warning=build.rs: Could not find a subdirectory containing ONNX Runtime libraries within {}", target_cache_dir.display());
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=proto/editing.proto"); // Add rerun trigger for proto

    // Conditionally compile based on features
    let server_enabled = cfg!(feature = "server");

    // --- Compile gRPC services --- 
    if server_enabled {
        println!("cargo:warning=vectordb-cli@1.5.0: build.rs: Compiling gRPC services (server feature enabled)...");
        
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
        let editing_descriptor_path = out_dir.join("editing_descriptor.bin");
        
        tonic_build::configure()
            .build_server(true)
            .build_client(false)
            .file_descriptor_set_path(&editing_descriptor_path) // Generate descriptor set
            .compile_protos(&["proto/editing.proto"], &["proto"])
            .map_err(|e| format!("Failed to compile gRPC services for server: {}", e))?;
            
        // Generate a Rust module that includes the descriptor as a constant
        let descriptor_mod = format!(
            r#"
            /// Generated file descriptor set for editing service
            pub const EDITING_FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("{}");
            "#,
            editing_descriptor_path.display().to_string().replace('\\', "\\\\")
        );
        
        let descriptor_mod_path = out_dir.join("editing_descriptor.rs");
        fs::write(&descriptor_mod_path, descriptor_mod)
            .map_err(|e| format!("Failed to write descriptor module: {}", e))?;
            
        println!("cargo:warning=vectordb-cli@1.5.0: build.rs: Generated editing descriptor at {}", editing_descriptor_path.display());
        println!("cargo:warning=vectordb-cli@1.5.0: build.rs: Finished compiling gRPC services.");
    } else {
        // No need to create dummy files if server feature is off, 
        // as the include! macro in src/grpc_generated/mod.rs is cfg-gated.
        println!("cargo:warning=build.rs: Skipping gRPC service compilation (server feature not enabled).");
    }

    // --- Rpath and Library Copy Logic for Linux/macOS ---
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        if let Some(source_lib_dir) = find_onnx_runtime_lib_dir() {
            let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
            
            // Bind the PathBuf to a variable to ensure it lives long enough
            let out_dir_path = PathBuf::from(&out_dir);
            
            // Calculate target directory relative to the longer-lived out_dir_path
            let target_profile_dir = out_dir_path
                .parent().expect("OUT_DIR has no parent?") // build/<crate>-<hash>
                .parent().expect("OUT_DIR has no parent parent?") // build/
                .parent().expect("OUT_DIR has no parent parent parent?"); // target/<profile>/

            let target_lib_dir = target_profile_dir.join("lib");
            if let Err(e) = fs::create_dir_all(&target_lib_dir) {
                 println!("cargo:warning=build.rs: Failed to create target library directory {}: {}. Skipping copy.", target_lib_dir.display(), e);
                 return Ok(());
            }

            // --- Copy Files Individually --- 
            let copy_options = fs_file::CopyOptions::new().overwrite(true);
            let mut files_copied = 0;
            let mut copy_errors = 0;

            println!(
                "cargo:warning=build.rs: Attempting to copy library files from {} to {}", 
                source_lib_dir.display(), 
                target_lib_dir.display()
            );
            
            match fs::read_dir(&source_lib_dir) {
                Ok(entries) => {
                    for entry_result in entries {
                        if let Ok(entry) = entry_result {
                            let source_path = entry.path();
                            if source_path.is_file() {
                                // Only copy .so or .dylib files
                                let extension = source_path.extension().and_then(|s| s.to_str());
                                if extension == Some("so") || extension == Some("dylib") {
                                    let target_path = target_lib_dir.join(entry.file_name());
                                    if let Err(e) = fs_file::copy(&source_path, &target_path, &copy_options) {
                                        println!(
                                            "cargo:warning=build.rs: Failed to copy {} to {}: {}", 
                                            source_path.display(), 
                                            target_path.display(), 
                                            e
                                        );
                                        copy_errors += 1;
                                    } else {
                                        files_copied += 1;
                                    }
                                }
                            }
                        }
                    }

                    if copy_errors > 0 {
                        println!(
                            "cargo:warning=build.rs: Finished copying with {} errors. {} files copied successfully.",
                            copy_errors,
                            files_copied
                        );
                    } else if files_copied > 0 {
                         println!(
                            "cargo:warning=build.rs: Successfully copied {} library files to {}",
                            files_copied,
                            target_lib_dir.display()
                        );
                    } else {
                         println!("cargo:warning=build.rs: No library files found to copy in {}.", source_lib_dir.display());
                    }

                },
                Err(e) => {
                     println!(
                        "cargo:warning=build.rs: Failed to read source library directory {}: {}. Skipping copy.", 
                        source_lib_dir.display(), 
                        e
                    );
                    return Ok(()); // Cannot proceed if source dir cannot be read
                }
            }

            // Only set RPATH if we successfully copied some files
            if files_copied > 0 && copy_errors == 0 {
                // Set RPATH relative to the executable
                let rpath_value = if cfg!(target_os = "macos") {
                    "@executable_path/lib"
                } else {
                    "$ORIGIN/lib"
                };
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", rpath_value);
                println!("cargo:warning=build.rs: Setting RPATH to: {}", rpath_value);
            } else {
                 println!("cargo:warning=build.rs: Skipping RPATH setup due to copy errors or no files copied.");
            }

        } else {
            println!("cargo:warning=build.rs: ONNX Runtime library directory not found in cache. Skipping library copy and RPATH setup.");
        }
    }

    // --- Remove the old Linux-specific block for copying individual provider libs ---
    // The logic above now handles copying all necessary libraries.
    
    Ok(())
}
