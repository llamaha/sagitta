# Multi-Tenancy End-to-End Test Plan for Vectordb MCP

This document describes a step-by-step plan to manually (or semi-automatically) verify multi-tenancy and API key isolation in the Vectordb MCP server, using HTTP API calls and Qdrant running on localhost:6334.

---

## **Prerequisites**
- MCP server is running and configured to use Qdrant at `localhost:6334`.
- You have access to the MCP HTTP API (replace `YOUR_MCP_PORT` with the actual port).
- `curl` is available for making HTTP requests.

---

## **Test Steps**

### **1. Create Two Tenants**

```bash
curl -X POST http://localhost:YOUR_MCP_PORT/api/v1/tenants \
  -H "Content-Type: application/json" \
  -d '{"name": "tenant_a"}'

curl -X POST http://localhost:YOUR_MCP_PORT/api/v1/tenants \
  -H "Content-Type: application/json" \
  -d '{"name": "tenant_b"}'
```
- **Expected:** Each returns a JSON object with a unique `tenant_id`.

### **2. Create API Keys for Each Tenant**

```bash
curl -X POST http://localhost:YOUR_MCP_PORT/api/v1/keys \
  -H "Content-Type: application/json" \
  -d '{"tenant_id": "TENANT_A_ID", "description": "Key for tenant A"}'

curl -X POST http://localhost:YOUR_MCP_PORT/api/v1/keys \
  -H "Content-Type: application/json" \
  -d '{"tenant_id": "TENANT_B_ID", "description": "Key for tenant B"}'
```
- **Expected:** Each returns a JSON object with an `api_key` (save these for later steps).

### **3. Add a Repository for Each Tenant**

```bash
curl -X POST http://localhost:YOUR_MCP_PORT/api/v1/repositories \
  -H "X-API-Key: API_KEY_FOR_TENANT_A" \
  -H "Content-Type: application/json" \
  -d '{"name": "repo_a", "url": "https://github.com/example/repo_a.git"}'

curl -X POST http://localhost:YOUR_MCP_PORT/api/v1/repositories \
  -H "X-API-Key: API_KEY_FOR_TENANT_B" \
  -H "Content-Type: application/json" \
  -d '{"name": "repo_b", "url": "https://github.com/example/repo_b.git"}'
```
- **Expected:** Each returns a JSON object confirming repository creation.

### **4. List Repositories for Each Tenant**

```bash
curl -X GET http://localhost:YOUR_MCP_PORT/api/v1/repositories \
  -H "X-API-Key: API_KEY_FOR_TENANT_A"

curl -X GET http://localhost:YOUR_MCP_PORT/api/v1/repositories \
  -H "X-API-Key: API_KEY_FOR_TENANT_B"
```
- **Expected:** Each tenant only sees their own repositories (`repo_a` for tenant A, `repo_b` for tenant B).

### **5. Attempt Cross-Tenant Access (Should Fail)**

```bash
# Try to access tenant B's repo with tenant A's key
curl -X GET http://localhost:YOUR_MCP_PORT/api/v1/repositories/repo_b \
  -H "X-API-Key: API_KEY_FOR_TENANT_A"

# Try to access tenant A's repo with tenant B's key
curl -X GET http://localhost:YOUR_MCP_PORT/api/v1/repositories/repo_a \
  -H "X-API-Key: API_KEY_FOR_TENANT_B"
```
- **Expected:** Each request should return 403 Forbidden or 404 Not Found (no data leakage).

### **6. (Optional) Sync or Query Operations**
- Repeat similar operations (e.g., sync, query) using each API key and verify isolation.

---

## **Security Review Checklist**
- [ ] API keys are stored hashed (not plaintext) in the backend.
- [ ] Error messages do not leak sensitive information (e.g., do not reveal existence of other tenants or keys).
- [ ] Rate limiting is enforced per API key/tenant.

---

## **Notes**
- Replace `YOUR_MCP_PORT`, `TENANT_A_ID`, `TENANT_B_ID`, `API_KEY_FOR_TENANT_A`, and `API_KEY_FOR_TENANT_B` with actual values from your environment.
- This plan can be adapted into a shell script or automated integration test if desired.
- For production, ensure HTTPS is enabled and secrets are managed securely.

---

*This test plan ensures that multi-tenancy and API key isolation are enforced at the API level, and that no cross-tenant data leakage is possible.* 