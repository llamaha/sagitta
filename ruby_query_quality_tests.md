# Ruby Query Quality & Relevance Tests

This document contains a set of targeted queries designed to manually assess the quality and relevance of search results for Ruby code within the `vectordb-cli`.

## Purpose

After adding relevant Ruby repositories and running `repo sync`, execute the queries below to verify that the tool returns meaningful and contextually relevant results for common Ruby and framework-specific concepts (like Rails and Discourse). This helps identify areas where parsing, chunking, embedding, or search logic might need improvement specifically for Ruby codebases.

## Prerequisites

1.  The `vectordb-cli` binary is compiled.
2.  Qdrant is running.
3.  The following repositories (or similar representative Ruby repos) have been added using `repo add --name <name> --url <url>`:
    *   `e2e-query-rails`: `https://github.com/rails/rails` (Ruby - Rails Framework)
    *   `e2e-query-discourse`: `https://github.com/discourse/discourse` (Ruby - Discourse Forum)
    *   *(Optional: `e2e-query-sinatra` from the main query test file if desired)*
4.  **Important:** Clean and sync the repositories **before** running the queries below, especially after changes to the parser or indexing logic:
    *   First, clear each repository's index: `repo clear --name <repo_name> -y`
    *   Then, sync each repository: `repo sync <repo_name>`
    *   *(Alternatively, script the clear and sync steps for all test repos)*

## Test Queries

Execute each command and evaluate the relevance of the top results (e.g., top 2). Ensure the `--lang ruby` flag is used.

**1. Ruby - Rails (Target: `e2e-query-rails`)**

*   **Concept:** Active Record Migrations (Defining schema changes).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'define active record migration' --name e2e-query-rails --lang ruby --limit 2
    ```
*   **Expected:** Code snippets showing `class CreateSomething < ActiveRecord::Migration[...]`, `create_table`, `add_column`, `t.string`, etc.

*   **Concept:** Rails Controller Actions (Handling web requests).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'rails controller action' --name e2e-query-rails --lang ruby --limit 2
    ```
*   **Expected:** Code snippets showing `class SomeController < ApplicationController`, methods like `def index`, `def show`, `def create`, `params[...]`, `render`, `redirect_to`.

*   **Concept:** Rails Routing (Mapping URLs to controllers/actions).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'rails routes definition' --name e2e-query-rails --lang ruby --limit 2
    ```
*   **Expected:** Code snippets from `config/routes.rb` showing `Rails.application.routes.draw do`, `get '/'`, `post '/users'`, `resources :posts`, `namespace :admin`.

**2. Ruby - Discourse (Target: `e2e-query-discourse`)**

*   **Concept:** Discourse Plugin System (Extending functionality).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'register discourse plugin' --name e2e-query-discourse --lang ruby --limit 2
    ```
*   **Expected:** Code related to plugin registration, potentially involving `Discourse::PluginRegistry`, `register_asset`, initializers in `plugin.rb` files.

*   **Concept:** Background Jobs (Using Sidekiq).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'discourse background job' --name e2e-query-discourse --lang ruby --limit 2
    ```
*   **Expected:** Code defining classes inheriting from `Jobs::Base` or `Jobs::Scheduled`, usage of `perform_async`, `perform_in`.

*   **Concept:** Model Callbacks (Hooks during object lifecycle).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'discourse model callback' --name e2e-query-discourse --lang ruby --limit 2
    ```
*   **Expected:** Code within model files (inheriting from `ApplicationRecord` or `ActiveRecord::Base`) showing `before_save`, `after_create`, `before_validation`, etc.

*   **Concept:** Model Validation (Ensuring data integrity).
*   **Command:**
    ```bash
    ./target/release/vectordb-cli repo query 'discourse model validation' --name e2e-query-discourse --lang ruby --limit 2
    ```
*   **Expected:** Code within model files showing `validates`, `presence: true`, `uniqueness: { scope: ... }`, `length: { maximum: ... }`. 