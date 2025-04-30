# Query Quality & Relevance Tests

This document contains a set of targeted queries designed to manually assess the quality and relevance of search results for different supported languages within the `vectordb-cli`.

## Purpose

After adding repositories and running `repo sync`, execute the queries below to verify that the tool returns meaningful and contextually relevant results for code-specific concepts in each language. This helps identify areas where parsing, chunking, embedding, or search logic might need improvement.

## Prerequisites

1.  The `vectordb-cli` binary is compiled.
2.  Qdrant is running.
3.  The following repositories (or similar representative repos) have been added using `repo add --name <name> --url <url>`:
    *   `e2e-query-ripgrep`: `https://github.com/BurntSushi/ripgrep` (Rust)
    *   `e2e-query-rustbook`: `https://github.com/rust-lang/book` (Markdown, YAML - Keep for non-Rust tests)
    *   `e2e-query-flask`: `https://github.com/pallets/flask` (Python)
    *   `e2e-query-gin`: `https://github.com/gin-gonic/gin` (Go)
    *   `e2e-query-tsnode`: `https://github.com/microsoft/TypeScript-Node-Starter` (TypeScript)
    *   `e2e-query-sinatra`: `https://github.com/sinatra/sinatra` (Ruby)
    *   *(Optional: `e2e-xxxx-spoon` from main E2E test for fallback/HTML)*
4.  **Important:** Clean and sync the repositories **before** running the queries below, especially after changes to the parser or indexing logic:
    *   First, clear each repository's index: `repo clear --name <repo_name> -y`
    *   Then, sync each repository: `repo sync <repo_name>`
    *   *(Alternatively, script the clear and sync steps for all test repos)*

## Test Queries

Execute each command and evaluate the relevance of the top results (e.g., top 2).

**1. Rust (Target: ripgrep)**

*   **Concept:** Implementing a specific trait (`From`).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'implementing the From trait' --name e2e-query-ripgrep --lang rust --limit 2
    ```
*   **Expected:** Code snippets showing `impl From<...> for ...` or related discussions.

**2. Markdown (Target: Rust Book)**

*   **Concept:** Explanation of a core language feature (Ownership).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'what is ownership in rust?' --name e2e-query-rustbook --lang markdown --limit 2
    ```
*   **Expected:** Sections from the book defining or explaining ownership.

**3. Python (Target: Flask)**

*   **Concept:** Usage or explanation of the Flask request context.
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'flask request context' --name e2e-query-flask --lang python --limit 2
    ```
*   **Expected:** Code snippets showing `from flask import request`, `request.`, or functions handling request context.

**4. Go (Target: Gin)**

*   **Concept:** Using router groups in the Gin framework.
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'gin router group' --name e2e-query-gin --lang go --limit 2
    ```
*   **Expected:** Code showing `router.Group(...)` or related functions/methods.

**5. TypeScript (Target: TS Node Starter)**

*   **Concept:** Defining or using Express.js controllers.
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'express controller' --name e2e-query-tsnode --lang typescript --limit 2
    ```
*   **Expected:** TypeScript code defining classes or functions used as controllers (e.g., handling `req, res`).

**6. Ruby (Target: Sinatra)**

*   **Concept:** How Sinatra handles request routing.
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'sinatra request routing' --name e2e-query-sinatra --lang ruby --limit 2
    ```
*   **Expected:** Ruby code showing `get '/' do ... end`, `post ...`, or modules related to routing/requests.

**7. YAML (Target: Rust Book)**

*   **Concept:** Configuration for GitHub Actions workflows.
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'github actions workflow' --name e2e-query-rustbook --lang yaml --limit 2
    ```
*   **Expected:** Content from `.github/workflows/*.yml` files. 