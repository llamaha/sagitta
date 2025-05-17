# Multi-Tenancy, API Keys, and Tenant Isolation in VectorDB-MCP

This document outlines how multi-tenancy, API key management, and data isolation are implemented within the VectorDB-MCP server.

## 1. Multi-Tenancy Model

VectorDB-MCP supports a multi-tenant architecture, allowing different users or organizations to use the service in an isolated manner.

*   **Tenants**: Each distinct entity (user, team, organization) is represented as a **Tenant**. Tenants are identified by a unique `tenant_id` (a UUID v4 string).
*   **Resources**: Key resources like Repositories are associated with a specific `tenant_id`. API keys are also issued on a per-tenant basis.
*   **Data Isolation**: The primary mechanism for data isolation at the storage level (Qdrant) is through **collection naming conventions**. Qdrant collections incorporate the `tenant_id`, ensuring that one tenant cannot accidentally access another tenant's data.

## 2. API Key Management

Secure access to the VectorDB-MCP server, especially for programmatic clients and MCP tools, is managed through API keys.

### a. API Key Structure

Each API key has the following properties:
*   `id`: An internal unique identifier for the key (UUID).
*   `key`: The secret API key string itself. It is prefixed with `vdb_sk_` (e.g., `vdb_sk_a1b2c3d4e5f6...`). This is the value clients must provide.
*   `tenant_id`: The ID of the tenant to whom this key belongs. All operations performed using this key will be scoped to this tenant.
*   `user_id`: (Optional) An identifier for a user within the tenant that this key might be associated with.
*   `description`: (Optional) A human-readable description for the key.
*   `scopes`: A list of strings defining the permissions granted by this key (e.g., `manage:tenants`, `read:repository`). Scope enforcement details are evolving.
*   `created_at`: Timestamp of when the key was created.
*   `expires_at`: (Optional) Timestamp of when the key will expire.
*   `last_used_at`: (Optional) Timestamp of when the key was last used.
*   `revoked`: A boolean flag indicating if the key has been revoked. Revoked keys are invalid.

### b. Bootstrap Admin API Key

For initial server setup and administration (like creating the first tenants), an admin API key can be bootstrapped using the environment variable:
*   `VECTORDB_BOOTSTRAP_ADMIN_KEY=your_chosen_admin_key_value`

When the server starts with this environment variable set, if the key doesn't already exist, it will be created with:
*   `tenant_id`: `"admin_tenant"` (a default tenant for administrative purposes)
*   `user_id`: `"admin_user"`
*   `description`: "Bootstrap Admin Key"
*   `scopes`: Typically includes `manage:tenants` or similar broad administrative permissions.

This key is essential for performing initial administrative tasks via the HTTP API.

### c. Managing API Keys (HTTP REST API)

API keys (for tenants other than the bootstrap admin, or additional keys for any tenant) are managed via HTTP REST endpoints, typically protected by an authenticated user with appropriate permissions (e.g., the bootstrap admin key or another key with key management scopes).

*   **Create API Key**: `POST /api/v1/keys/`
    *   Requires `tenant_id` in the request body.
    *   Returns the full `ApiKey` object, including the `key` value. **This is the only time the raw key is returned.**
*   **List API Keys**: `GET /api/v1/keys/`
    *   Can be filtered by `tenant_id`.
    *   Returns a list of `ApiKeyInfo` objects, which include a `key_preview` (e.g., `vdb_sk_a1b2...`) but **omits the full secret key**.
*   **Revoke/Delete API Key**: `DELETE /api/v1/keys/:key_id`
    *   Marks the specified API key as `revoked`.

## 3. Tenant Management (HTTP REST API)

Tenants themselves are also managed via an HTTP REST API, typically requiring administrative privileges (e.g., using the bootstrap admin API key).

*   **Create Tenant**: `POST /api/v1/tenants/`
    *   Request body includes `name` for the tenant.
    *   Returns the created `Tenant` object, including its generated `id`.
*   **List Tenants**: `GET /api/v1/tenants/`
*   **Get Tenant**: `GET /api/v1/tenants/:id`
*   **Update Tenant**: `PUT /api/v1/tenants/:id`
*   **Delete Tenant**: `DELETE /api/v1/tenants/:id` (Note: Ensure data cleanup procedures are considered).

## 4. Authentication Mechanisms (HTTP Transport)

When requests are made over HTTP/HTTPS, the `auth_middleware` attempts to authenticate and establish a tenant context:

### a. API Key Authentication

*   Clients should provide their API key in the `X-API-Key` HTTP header.
*   The middleware validates the key against the `ApiKeyStore`:
    *   Checks if the key exists.
    *   Checks if it's valid (not revoked, not expired).
*   If valid, the `tenant_id` and `scopes` from the API key are used to form an `AuthenticatedUser` context for the request.
*   API key usage (last used timestamp) is recorded.

### b. OAuth 2.0 Bearer Token Authentication

*   Clients can provide an OAuth 2.0 Bearer token in the `Authorization: Bearer <token>` HTTP header.
*   The middleware:
    1.  Validates the token using the configured OAuth provider's introspection endpoint.
    2.  If the token is valid, it fetches user information (e.g., `sub`, `email`) from the user info endpoint.
    3.  It then attempts to map the OAuth user's subject identifier (`sub`) to a `tenant_id` using the `OAuthUserTenantMappingStore`.
    4.  If a mapping exists, that `tenant_id` is used.
    5.  If no mapping is found, a placeholder `tenant_id` (`__unmapped_oauth_tenant__`) is associated with the request. Access for such users depends on how this placeholder tenant is treated.
*   Scopes might be derived from the OAuth token or the user mapping.
*   The result is an `AuthenticatedUser` context.

## 5. Tenant Isolation in MCP Operations

MCP operations like `repository/add`, `repository/sync`, `query`, etc., respect tenant boundaries.

### a. Repository-Tenant Association

*   Each repository registered with VectorDB-MCP is associated with a `tenant_id`. This association is stored in the `RepositoryConfig` for that repository.
*   When a new repository is added (`repository/add` command), the `tenant_id` of the authenticated context (e.g., from the API key used) is assigned to the new repository.

### b. Access Control

*   For operations on repositories (list, remove, sync, query, search files, view files), the system compares:
    1.  The `acting_tenant_id`: The tenant ID of the authenticated context making the request (derived from API key or OAuth mapping).
    2.  The `repository_tenant_id`: The tenant ID stored in the configuration of the target repository.
*   If these tenant IDs do not match, the operation is denied with an "Access Denied" error. This prevents one tenant from accessing or modifying another tenant's repositories.
*   A server-wide default `tenant_id` (set in `AppConfig.tenant_id`) can serve as a fallback if no tenant context is available from authentication, but repository-specific tenant IDs always take precedence for access control.

### c. Qdrant Collection Naming for Data Isolation

*   The core of data isolation in Qdrant (the vector database) is achieved by naming collections in a tenant-specific way.
*   The function `vectordb_core::repo_helpers::get_collection_name(...)` is used to generate collection names. It typically combines:
    *   A global prefix (e.g., `repo_` from `AppConfig.performance.collection_name_prefix`).
    *   The `tenant_id` of the repository.
    *   The `repository_name`.
*   Example: `repo_mytenant123_myrepository`
*   This ensures that all Qdrant operations (indexing, searching) for a given repository are performed only on the collection belonging to that repository's tenant.

## 6. OAuth User-to-Tenant Mapping

For scenarios where users authenticate via an external OAuth 2.0 provider, a mapping system allows associating these external identities with internal tenants.

*   **Mapping**: An `OAuthUserTenantMapping` links an OAuth user's subject identifier (`sub` claim) to a `tenant_id` within VectorDB-MCP.
*   **Management**: These mappings can be managed by administrators via HTTP REST API endpoints:
    *   `POST /api/v1/admin/oauth-mappings/`: Add a new mapping.
    *   `GET /api/v1/admin/oauth-mappings/`: List mappings.
    *   `DELETE /api/v1/admin/oauth-mappings/:oauth_sub`: Remove a mapping.
*   **Usage**: When a user authenticates via OAuth, the `auth_middleware` uses this store to find the appropriate `tenant_id` for the session.

## 7. Considerations for Stdio and HTTP/SSE Transports

*   **HTTP REST API**: The multi-tenancy features described (API key auth, OAuth, tenant isolation for management and repository operations) are fully effective when interacting with the dedicated HTTP REST APIs (e.g., `/api/v1/tenants/`, `/api/v1/keys/`) and if MCP methods are invoked via an HTTP handler that properly utilizes the `AuthenticatedUser` context from the `auth_layer` middleware.
*   **Stdio Transport**: When `vectordb-mcp` runs in `stdio` mode (e.g., managed by Cursor), it uses a different request processing path (`Server::handle_request`). This path does **not** automatically benefit from the Axum-based `auth_layer` middleware.
    *   Tenant context in Stdio typically relies on:
        1.  A `tenant_id` potentially passed within the MCP request parameters themselves (e.g., `params.tenant_id` for `repository/add`).
        2.  The server-wide default `tenant_id` configured in `AppConfig.tenant_id`.
    *   While access control checks against `RepositoryConfig.tenant_id` still apply using this fallback tenant ID, **user-specific tenancy based on an API key provided over Stdio is not directly supported by the current Stdio transport layer.**
*   **HTTP/SSE Transport (`/sse`, `/message`)**: The current implementation of the HTTP/SSE transport for MCP methods also routes messages through `Server::handle_request`. While the initial SSE connection might be authenticated by `auth_layer`, this authenticated context (and thus the specific `tenant_id` from an API key) is not propagated to the `Server::handle_request` calls for individual MCP messages sent over that SSE session. It would also rely on MCP parameters or the server default `tenant_id`.

**Future Enhancements**: To enable full API key-based, per-request tenancy for MCP methods over Stdio or HTTP/SSE, the transport layers would need to be modified to securely pass and interpret API keys or tenant identifiers for each MCP request and then supply this context to the core handlers.

## 8. Scope Enforcement

*   API keys can be created with a list of `scopes` (e.g., `manage:tenants`, `read:repository:myrepo`, `write:repository`).
*   The `auth_middleware` extracts these scopes into the `AuthenticatedUser` context.
*   **Current Status**: While the infrastructure for scopes exists, detailed enforcement logic for fine-grained scopes (e.g., allowing a key to only access specific repositories or perform only read actions) within each handler is an area that may see further development. The bootstrap admin key relies on a broad scope like `manage:tenants`. Other operations currently primarily rely on the tenant ID match for access control. 