# Vectordb Multi-Tenancy and API Key Plan (Focused for Corporate Deployment)

## Goal for this Iteration:
Enable deployment within a corporate environment where users, identified by API keys, can perform repository operations (sync, query, tools) via the MCP server, with data isolation between different API keys if they represent different "users" or logical "tenants". Each API key will be associated with a specific tenant_id.

## Phase 1: Core and Configuration Changes (Largely Completed)

-   [x] Modify `RepositoryConfig` to include `tenant_id: Option<String>`.
-   [x] Update `AppConfig` serialization/deserialization.
-   [x] Ensure `vectordb-core` functions (`repo_add`, `repo_sync`, `qdrant_ops`, `indexing`) correctly use `tenant_id` to construct Qdrant collection names (e.g., `{tenant_id}_{repo_name}`).
    -   `get_collection_name` utility requires `tenant_id`.
-   [x] Update relevant CLI commands (`repo add`, `repo sync`, etc.) in `vectordb-cli` to manage/pass `tenant_id` (for admin/setup purposes).
-   [x] Add basic HTTP Server features (TLS, Health Check, Secure Headers, CORS Policy).

## Phase 2: Multi-Tenancy & API Key Essentials (Core Focus)

### 2.1. Tenant Representation (Sufficient for Admin Setup)
-   [x] Define Tenant Model: `id`, `name`, `status`, `created_at`, `updated_at`, `metadata`. (Sufficient for admin to create logical tenants)
-   [x] Implement Tenant Store: Trait and an initial in-memory implementation. (Sufficient for now)
-   [x] Expose Tenant CRUD via MCP HTTP API (e.g., `POST /api/v1/tenants`).
    -   **Focus:** For admin use to set up tenants that API keys will be associated with. End-user self-service tenant management is deferred.
-   [Deferred] CLI commands for end-user tenant management.
-   [Deferred] Basic Admin UI for tenant management.

### 2.2. Data Isolation via Tenant ID in MCP (Largely Completed)
-   [x] **Tenant isolation checks in MCP handlers:**
    -   [x] `AuthenticatedUser` (from `auth_middleware.rs`) MUST provide the `tenant_id` associated with the API key.
    -   [x] MCP handlers (`repository.rs`, `query.rs`, `search_file.rs`, `view_file.rs`, tool handlers) use this `tenant_id` to operate *only* on resources (repositories, Qdrant collections) matching that `tenant_id`.
-   [x] Ensure Qdrant collections are named to include `tenant_id` (via core changes in Phase 1).

### 2.3. API Key Management (Critical Path)
-   [x] Define API Key Model: `key_id`, `key_hash`, **`tenant_id: String` (Non-optional, critical for linking to a tenant)**, `user_id` (optional), `description`, `scopes` (deferring granular scopes for now), `expires_at`, `last_used_at`, `created_at`, `status`.
    -   Status: `tenant_id` is now `String` in `ApiKey` struct and store.
-   [x] Implement API Key Store: Trait and an initial in-memory implementation.
-   [x] Expose API Key CRUD via MCP HTTP API:
    -   `POST /api/v1/keys` (or `/api/v1/tenants/{tenant_id}/keys`): **Must require `tenant_id` for association during creation.**
        -   Status: `CreateApiKeyRequest` now requires `tenant_id: String`.
    -   `GET /api/v1/keys` (or `/api/v1/tenants/{tenant_id}/keys`): List keys, perhaps filterable by tenant for admin, or restricted to own tenant's keys.
    -   `DELETE /api/v1/keys/{key_id}`: Revoke key. Access control needed (e.g., only owner tenant or admin).
    -   **Focus:** Admin ability to create keys for tenants. Tenant self-service key management UI is deferred.
-   [x] Basic Rate Limiting per API Key/Tenant (Current implementation is a good starting point).
-   [Deferred] Granular scopes/permissions for API keys. For now, an API key grants full access to its associated tenant's resources for supported operations.
-   [Deferred] CLI commands for end-user API key management.

## Phase 3: Authentication Refinements (Critical Path for API Keys)

-   **Authentication Middleware (`auth_middleware.rs`):**
    -   [x] Supports API keys (e.g., `X-API-Key` header).
    -   [x] **`[COMPLETED]` Enhance to populate `AuthenticatedUser` with the correct `tenant_id` derived from the validated API key.**
        -   Status: `AuthenticatedUser.tenant_id` is now `String` and populated from `ApiKey.tenant_id`. API Key flow is complete.
    -   [x] API keys are validated against the `ApiKeyStore` (hashed comparison).
    -   [x] **`[NEWLY COMPLETED]` OAuth specific refinements for tenant context:**
        -   [x] Core infrastructure for mapping OAuth user 'sub' to a `tenant_id`.
        -   [x] Admin API endpoints (`/api/v1/admin/oauth-mappings`) for managing these mappings.
        -   [x] `auth_middleware.rs` now uses this mapping to populate `AuthenticatedUser.tenant_id` for OAuth users. If no mapping exists, a placeholder `__unmapped_oauth_tenant__` is used.
-   [Deferred] Detailed Scopes / Permissions system.

## Phase 4: UI & CLI (Minimal Admin CLI, User UI/CLI Deferred)

-   [Deferred] End-user CLI updates for tenant context.
-   [Deferred] Admin UI / End-user UI.
-   **Consider (Low Priority for now):** Basic admin CLI to create a tenant and generate an API key for it.

## Phase 5: Testing (Ongoing, Core Functionality Tested)

-   [x] Unit tests for tenant isolation logic in MCP handlers.
-   [x] Unit tests for API key generation, validation (basic), and management logic.
-   [x] Unit tests for tenant management (store and handlers).
-   [x] Unit tests for `auth_middleware.rs` to verify `AuthenticatedUser.tenant_id` population.
-   `[TODO]` Integration tests covering:
    - API key authentication providing correct `tenant_id` to handlers.
    - End-to-end tenant isolation for `repository/*`, `query`, and `tool/*` operations using different API keys.
-   [Deferred] Broader security testing, performance testing for multi-tenant.

## Phase 6: SaaS Monetization & Business Features (All Deferred)
- All items in this phase are deferred for the initial corporate deployment goal.

## Future Enhancements (Post-Corporate Deployment MVP)
- Granular Scopes and Permissions System
- Full CLI for User Tenant and API Key Management
- Admin & User Web UIs
- Persistent DB for Tenants and API Keys (e.g., PostgreSQL)
- Advanced Rate Limiting & Quotas
- OAuth refinements for tenant mapping
- Audit Logging
- Full SaaS features (Subscription, Billing, etc.)

---
*This plan is focused on achieving a deployable multi-user (via API keys) system with data isolation.* 