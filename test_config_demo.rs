use sagitta_search::config::{AppConfig, PerformanceConfig};

fn main() {
    println!("=== Default Config Serialization Demo ===");
    
    // Create a default config
    let default_config = AppConfig::default();
    let serialized = toml::to_string_pretty(&default_config).expect("Failed to serialize");
    
    println!("Default config serialized:");
    println!("{}", serialized);
    
    // Check if vector_dimension appears in the output
    if serialized.contains("vector_dimension") {
        println!("❌ vector_dimension appears in default config (should be omitted)");
    } else {
        println!("✅ vector_dimension omitted from default config (as expected)");
    }
    
    println!("\n=== Custom Config Serialization Demo ===");
    
    // Create a config with non-default vector_dimension
    let mut custom_config = AppConfig::default();
    custom_config.performance.vector_dimension = 512;
    let custom_serialized = toml::to_string_pretty(&custom_config).expect("Failed to serialize");
    
    println!("Custom config with vector_dimension=512:");
    println!("{}", custom_serialized);
    
    // Check if vector_dimension appears in the output
    if custom_serialized.contains("vector_dimension = 512") {
        println!("✅ vector_dimension appears in custom config (as expected)");
    } else {
        println!("❌ vector_dimension missing from custom config (should be present)");
    }
    
    println!("\n=== Performance Config Only Demo ===");
    
    // Test just the performance config
    let default_perf = PerformanceConfig::default();
    let perf_serialized = toml::to_string_pretty(&default_perf).expect("Failed to serialize");
    
    println!("Default PerformanceConfig:");
    println!("{}", perf_serialized);
    
    if perf_serialized.contains("vector_dimension") {
        println!("❌ vector_dimension appears in default PerformanceConfig");
    } else {
        println!("✅ vector_dimension omitted from default PerformanceConfig");
    }
} 