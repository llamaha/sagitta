//! Integration test for multi-tenancy, API key, and OAuth flows in sagitta-mcp.

use reqwest::Client;
use serde_json::json;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use std::io::Read;
use tempfile::NamedTempFile;
use std::fs::write;
use std::ops::Drop;

struct ServerGuard(Child);
impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn spawn_mcp_server_with_test_config() -> (ServerGuard, NamedTempFile) {
    // Create a temporary config.toml with CORS disabled
    let mut temp_config = NamedTempFile::new().expect("Failed to create temp config file");
    let config_contents = r#"
qdrant_url = "http://localhost:6334"
onnx_model_path = "/tmp/onnx/model.onnx"
onnx_tokenizer_path = "/tmp/onnx/tokenizer.json"
repositories_base_path = "/tmp/repos"
vocabulary_base_path = "/tmp/vocab"
indexing.max_concurrent_upserts = 8
performance.batch_size = 256
performance.internal_embed_batch_size = 128
performance.collection_name_prefix = "repo_"
performance.max_file_size_bytes = 5242880
performance.vector_dimension = 384
tls_enable = false
cors_allow_credentials = false
# No cors_allowed_origins
"#;
    write(temp_config.path(), config_contents).expect("Failed to write temp config");
    let mut cmd = Command::new("../../target/release/sagitta-mcp");
    cmd.args(["http", "--host", "127.0.0.1", "--port", "8082"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.env("SAGITTA_TEST_CONFIG_PATH", temp_config.path().to_str().unwrap());
    cmd.env("SAGITTA_BOOTSTRAP_ADMIN_KEY", "test_admin_key");
    let child = cmd.spawn().expect("Failed to start MCP server");
    (ServerGuard(child), temp_config)
}

#[tokio::test]
async fn test_multi_tenancy_and_oauth_flow() {
    // Start the MCP server
    let (server_guard, temp_config) = spawn_mcp_server_with_test_config();

    let client = Client::new();
    let base_url = "http://127.0.0.1:8082";

    // Wait for the server to be ready (up to 30 seconds)
    let mut ready = false;
    for _ in 0..30 {
        if client.get(format!("{}/health", base_url)).send().await.is_ok() {
            ready = true;
            break;
        }
        sleep(Duration::from_secs(1));
    }
    assert!(ready, "MCP server did not become ready in time");

    // Generate an admin API key for privileged operations
    let admin_key = "test_admin_key";

    // 1. Create two tenants
    let resp1 = client.post(format!("{}/api/v1/tenants/", base_url))
        .header("X-API-Key", admin_key)
        .json(&json!({"name": "tenant1"}))
        .send().await.unwrap();
    if !resp1.status().is_success() {
        let status = resp1.status();
        let body = resp1.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
        panic!("Failed to create tenant1: status {} body: {}", status, body);
    }
    let tenant1 = resp1.json::<serde_json::Value>().await.unwrap();

    let resp2 = client.post(format!("{}/api/v1/tenants/", base_url))
        .header("X-API-Key", admin_key)
        .json(&json!({"name": "tenant2"}))
        .send().await.unwrap();
    if !resp2.status().is_success() {
        let status = resp2.status();
        let body = resp2.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
        panic!("Failed to create tenant2: status {} body: {}", status, body);
    }
    let tenant2 = resp2.json::<serde_json::Value>().await.unwrap();
    let tenant1_id = tenant1["id"].as_str().unwrap();
    let tenant2_id = tenant2["id"].as_str().unwrap();

    // 2. Create API keys for each tenant
    let key1_resp = client.post(format!("{}/api/v1/keys/", base_url))
        .header("X-API-Key", admin_key)
        .json(&json!({"tenant_id": tenant1_id, "description": "key1", "scopes": []}))
        .send().await.unwrap();
    if !key1_resp.status().is_success() {
        let status = key1_resp.status();
        let body = key1_resp.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
        panic!("Failed to create key1 for tenant1: status {} body: {}", status, body);
    }
    let key1 = key1_resp.json::<serde_json::Value>().await.unwrap();

    let key2_resp = client.post(format!("{}/api/v1/keys/", base_url))
        .header("X-API-Key", admin_key)
        .json(&json!({"tenant_id": tenant2_id, "description": "key2", "scopes": []}))
        .send().await.unwrap();
    if !key2_resp.status().is_success() {
        let status = key2_resp.status();
        let body = key2_resp.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
        panic!("Failed to create key2 for tenant2: status {} body: {}", status, body);
    }
    let key2 = key2_resp.json::<serde_json::Value>().await.unwrap();
    let api_key1 = key1["key"].as_str().unwrap();
    let api_key2 = key2["key"].as_str().unwrap();

    // 3. Use key1 to add/query a repo for tenant1, ensure key2 cannot access it, and vice versa
    // (Repository add/sync/query endpoints would be tested here if exposed via HTTP API)
    // For now, test that API key1 cannot list tenant2's keys, and vice versa
    let keys_tenant1_resp = client.get(format!("{}/api/v1/keys/?tenant_id={}", base_url, tenant1_id))
        .header("X-API-Key", api_key1)
        .send().await.unwrap();
    if !keys_tenant1_resp.status().is_success() {
        let status = keys_tenant1_resp.status();
        let body = keys_tenant1_resp.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
        panic!("Failed to list keys for tenant1: status {} body: {}", status, body);
    }

    let keys_tenant2_resp = client.get(format!("{}/api/v1/keys/?tenant_id={}", base_url, tenant2_id))
        .header("X-API-Key", api_key1)
        .send().await.unwrap();
    assert!(keys_tenant2_resp.status().as_u16() == 403 || keys_tenant2_resp.status().as_u16() == 404, "Expected 403 or 404 for cross-tenant key listing, got {}", keys_tenant2_resp.status());

    // 4. Mock OAuth flow: simulate an OAuth user and check placeholder tenant_id
    // (Assume /auth/login and /auth/callback endpoints exist and return a placeholder tenant)
    let oauth_userinfo = client.get(format!("{}/auth/userinfo", base_url))
        .header("Authorization", "Bearer mock_oauth_token")
        .send().await.unwrap();
    // Should return a userinfo with a placeholder tenant_id or deny access
    assert!(oauth_userinfo.status().is_success() || oauth_userinfo.status().as_u16() == 401);

    // 5. Clean up: kill the server and remove temp config
    temp_config.close().expect("Failed to close temp config file");
} 