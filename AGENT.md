# Hulu Payments Agent Guidelines

## Commands
- Build: `cargo build`
- Test all: `cargo test`
- Test single: `cargo test test_name`
- Check/lint: `cargo check`
- Run server: `cargo run`

## Architecture
- Actix-web REST API with PostgreSQL database
- Entry point: `src/main.rs` (binary), `src/lib.rs` (library)
- Structure: handlers/ (controllers), services/ (business logic), models/ (data types), routes/ (routing), db/ (database), dtos/ (data transfer objects)
- Database: PostgreSQL with SQLX migrations in `migrations/`
- External APIs: Stripe payments, Cloudflare Turnstile verification
- Testing: Integration tests in `tests/`, unit tests use `#[actix_web::test]`

## Code Style
- File headers: Include copyright header with creation date and author
- Imports: Group std, external crates, then local modules
- Error handling: Use `anyhow::Result` for main functions, custom errors for API responses
- Types: Use `serde` for JSON serialization, `sqlx` types for database
- Naming: snake_case for variables/functions, PascalCase for types/structs
- API responses: Wrap in `ApiResponse<T>` struct for consistency
