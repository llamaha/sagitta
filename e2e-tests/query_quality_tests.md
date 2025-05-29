# Query Quality & Relevance Tests (Revised)

This document contains human-like search queries designed to assess the quality and relevance of search results for different programming languages within `sagitta-cli`. These queries are optimized for hybrid (dense + sparse vector) search capabilities and test a variety of element types with appropriate language specifications.

## Purpose

After adding repositories and running `repo sync`, execute these queries to verify that the tool returns meaningful and contextually relevant results for specific features in each codebase. Each concept includes multiple query variations to exercise both semantic and keyword-based retrieval, with explicit testing of different element types and language specifications.

## Prerequisites

1. The `sagitta-cli` binary is compiled.
2. Qdrant is running.
3. The following repositories have been added using `repo add --name <name> --url <url>`:
   * `ripgrep-mcp-test`: `https://github.com/BurntSushi/ripgrep` (Rust)
   * `e2e-query-rustbook`: `https://github.com/rust-lang/book` (Markdown, YAML)
   * `e2e-query-flask`: `https://github.com/pallets/flask` (Python)
   * `e2e-query-gin`: `https://github.com/gin-gonic/gin` (Go)
   * `e2e-query-tsnode`: `https://github.com/microsoft/TypeScript-Node-Starter` (TypeScript)
   * `e2e-query-sinatra`: `https://github.com/sinatra/sinatra` (Ruby)
4. **Important:** Clean and sync repositories before running the queries:
   * `repo clear --name <repo_name> -y`
   * `repo sync <repo_name>`

## Test Queries (Optimized for Human-Like Hybrid Search)

### 1. Rust (Target: ripgrep)
**Feature:** regex handling in ripgrep

- `how does ripgrep handle large regex patterns?` --lang rust
- `regex size limit` --type constant --lang rust
- `Grep struct implementation` --type struct --lang rust
- `regex matching trait` --type trait --lang rust
- `pattern matching functions` --type function --lang rust
- `regex error handling` --type enum --lang rust
- `impl Matcher for RegexMatcher` --type impl --lang rust
- `what happens when my regex is too big for ripgrep?` --lang rust

### 2. Markdown (Target: Rust Book)
**Feature:** Error handling patterns in Rust

- `what's the best way to handle errors in Rust?` --lang markdown
- `custom error types` --type file_chunk --lang markdown
- `implementing Error trait` --type code_block --lang markdown
- `Result enum examples` --type heading --lang markdown
- `propagating errors` --type section --lang markdown
- `when should I use unwrap in Rust?` --lang markdown
- `? operator usage` --type paragraph --lang markdown
- `error handling best practices Rust` --lang markdown

### 3. Python (Target: Flask)
**Feature:** Blueprint functionality in Flask

- `how do Flask blueprints work?` --lang python
- `Blueprint class definition` --type class --lang python
- `register_blueprint method` --type method --lang python
- `blueprint decorator implementation` --type function --lang python
- `Blueprint.route` --type decorator --lang python
- `blueprint error handlers` --type method --lang python
- `Blueprint.__init__` --type method --lang python
- `nested blueprints in Flask` --lang python

### 4. Go (Target: Gin)
**Feature:** Middleware and routing in Gin

- `how to write custom middleware in Gin?` --lang go
- `Engine struct` --type struct --lang go
- `RouterGroup interface` --type interface --lang go
- `Handler method implementation` --type method --lang go
- `Context struct fields` --type struct --lang go
- `middleware chain execution` --type function --lang go
- `middleware.Auth` --type function --lang go
- `route parameter handling` --type method --lang go

### 5. TypeScript (Target: TS Node Starter)
**Feature:** Authentication and type system

- `how is authentication implemented in this TypeScript starter?` --lang typescript
- `User interface definition` --type interface --lang typescript
- `UserDocument type` --type type --lang typescript
- `AuthController class` --type class --lang typescript
- `passport strategy` --type variable --lang typescript
- `authenticate middleware` --type function --lang typescript
- `login form validation` --type method --lang typescript
- `JWT implementation` --type class --lang typescript

### 6. JavaScript (Target: TS Node Starter)
**Feature:** Express routing and controllers

- `express route handlers` --lang javascript
- `app.use middleware chain` --type function --lang javascript
- `router.get implementation` --type function --lang javascript
- `controller error handling` --type function --lang javascript
- `request validation` --type function --lang javascript
- `response formatting` --type method --lang javascript
- `async route handler` --type function --lang javascript
- `how are express routes organized in this project?` --lang javascript

### 7. YAML (Target: Rust Book or Any CI Config)
**Feature:** CI/CD configuration patterns

- `GitHub Actions workflow configuration` --lang yaml
- `CI pipeline for Rust projects` --lang yaml
- `build matrix definition` --type mapping --lang yaml
- `dependency caching` --type mapping --lang yaml
- `test job configuration` --type sequence --lang yaml
- `environment variables` --type mapping --lang yaml
- `workflow triggers` --type mapping --lang yaml
- `what's a good CI/CD setup for a Rust project?` --lang yaml

### 8. Markdown (Target: Rust Book)
**Feature:** Concurrency patterns in Rust

- `concurrency vs parallelism in Rust` --lang markdown
- `thread spawning examples` --type code_block --lang markdown
- `mutex implementation` --type code_block --lang markdown
- `channel communication` --type section --lang markdown
- `concurrency patterns` --type heading --lang markdown
- `Arc and Mutex patterns` --type file_chunk --lang markdown
- `data races prevention` --type paragraph --lang markdown
- `what's the difference between Send and Sync?` --lang markdown

This comprehensive query plan tests all supported languages:
- Rust: struct, trait, function, enum, impl, constant
- Markdown: file_chunk, code_block, heading, section, paragraph
- Python: class, method, function, decorator
- Go: struct, interface, method, function
- TypeScript: interface, type, class, variable, function, method
- JavaScript: function, method
- YAML: mapping, sequence, file_chunk

