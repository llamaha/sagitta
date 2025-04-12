use std::env;
use std::fs;
use std::path::PathBuf;
// use std::path::Path; // Removed unused import
// use std::process::Command; // Removed if not needed elsewhere

fn find_onnx_runtime_lib_dir() -> Option<PathBuf> {
    let home_dir = env::var("HOME").ok()?;
    let cache_base = PathBuf::from(home_dir).join(".cache/ort.pyke.io/dfbin");

    let target_triple = env::var("TARGET").ok()?;
    let target_cache_dir = cache_base.join(&target_triple);

    if !target_cache_dir.is_dir() {
        eprintln!("cargo:warning=ONNX Runtime cache directory not found at {}", target_cache_dir.display());
        return None;
    }

    let lib_name = if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "libonnxruntime.so" };

    if let Ok(entries) = fs::read_dir(&target_cache_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                 let lib_dir = path.join("onnxruntime/lib");
                 if lib_dir.is_dir() {
                    // Check if the actual library file exists in this directory
                    if lib_dir.join(lib_name).exists() {
                        // Found a directory that contains the library file
                        return Some(lib_dir);
                    }
                 }
            }
        }
    } else {
        eprintln!("cargo:warning=Failed to read ONNX cache directory entries: {}", target_cache_dir.display());
        return None;
    }

    // If we iterated through all entries and didn't find a valid lib dir
    eprintln!("cargo:warning=Could not find a valid ONNX Runtime library directory containing {}", lib_name);
    None
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // --- Rpath logic for Linux/macOS ---
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        if let Some(lib_dir) = find_onnx_runtime_lib_dir() {
            let lib_name = if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "libonnxruntime.so" };
            let source_lib_path = lib_dir.join(lib_name);

            // We already confirmed source_lib_path exists in find_onnx_runtime_lib_dir
            // So, we can proceed directly with copying

            // let profile = env::var("PROFILE").expect("PROFILE not set"); // release or debug - UNUSED
            
            // Calculate target directory relative to OUT_DIR
            let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
            // Bind the PathBuf to ensure it lives long enough for the borrow
            let out_dir_path = PathBuf::from(&out_dir);
            let target_dir = out_dir_path
                .parent().expect("OUT_DIR has no parent?") // build/<crate>-<hash>
                .parent().expect("OUT_DIR has no parent parent?") // build/
                .parent().expect("OUT_DIR has no parent parent parent?"); // target/<profile>/
            
            // Create the lib directory right next to the final executable
            let dest_lib_dir = target_dir.join("lib");
            fs::create_dir_all(&dest_lib_dir).expect("Failed to create destination lib directory");

            let dest_lib_path = dest_lib_dir.join(lib_name);

            // Copy the library using fs_extra for robustness
             match fs_extra::file::copy(&source_lib_path, &dest_lib_path, &fs_extra::file::CopyOptions::new().overwrite(true)) {
                 Ok(_) => {
                     println!("cargo:warning=Copied {} to {}", source_lib_path.display(), dest_lib_path.display());

                     // Set RPATH relative to the executable in target/<profile>/ (not target/<profile>/deps)
                     let rpath_value = if cfg!(target_os = "macos") {
                         "@executable_path/lib" // Relative to binary location
                     } else {
                         "$ORIGIN/lib" // Relative to binary location (Linux)
                     };
                     println!("cargo:rustc-link-arg=-Wl,-rpath,{}", rpath_value);
                     println!("cargo:warning=Setting RPATH to: {}", rpath_value);
                }
                Err(e) => {
                     eprintln!("cargo:warning=Failed to copy ONNX Runtime library: {}", e);
                }
             }
        } else {
            // Warning already printed by find_onnx_runtime_lib_dir if it returned None
            eprintln!("cargo:warning=Skipping RPATH setup because ONNX Runtime library directory was not found or validated.");
        }
    }

    // --- Removed bindgen code block ---
} 