# Agent Guidelines

## Meta Rules
- **Always update this AGENT.md file when discovering new patterns, rules, or conventions**

## Commands
- Build: `cargo build`
- Test all: `cargo test`
- Test single: `cargo test test_name`
- Check/lint: `cargo check`
- Run server: `cargo run`
- SQLx prepare (after query changes): `DATABASE_URL=postgresql://127.0.0.1:5432/campus_pilot cargo sqlx prepare`
  - **IMPORTANT**: Always run this command after adding or modifying `query!` or `query_as!` macros
  - Commit the `.sqlx/` directory to version control

## Architecture
- Actix-web REST API with PostgreSQL database
- Entry point: `src/main.rs` (binary), `src/lib.rs` (library)
- Structure: handlers/ (controllers), services/ (business logic), models/ (data types), routes/ (routing), db/ (database), dtos/ (data transfer objects)
- Database: PostgreSQL with SQLX migrations in `migrations/`
- Testing: Integration tests in `tests/`, unit tests use `#[actix_web::test]`

## Code Style
- File headers: Include copyright header with creation date and author
- Imports: Group std, external crates, then local modules
- Error handling: Use `anyhow::Result` for main functions, custom errors for API responses
- Types: Use `serde` for JSON serialization, `sqlx` types for database
- Naming: snake_case for variables/functions, PascalCase for types/structs
- API responses: Wrap in `ApiResponse<T>` struct for consistency

## SQL Style (PostgreSQL)
- All SQL keywords should use UPPERCASE (CREATE, SELECT, INSERT, UPDATE, DELETE, etc.)
- All PostgreSQL data types should be UPPERCASE (TEXT, INTEGER, BOOLEAN, UUID, etc.)
- All built-in functions should be UPPERCASE (NOW(), LOWER(), TO_JSONB(), etc.)
- All control flow keywords should be UPPERCASE (BEGIN, END, IF, THEN, ELSE, etc.)
- All trigger/function keywords should be UPPERCASE (RETURNS, LANGUAGE, EXECUTE, etc.)
- Column names and table names should remain lowercase with underscores
- String literals and comments can remain as-is
- All create statements should include IF NOT EXISTS
- All tables should have deleted_at, created_at, and updated_at fields
