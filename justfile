set dotenv-load

# List available commands
default:
    @just --list

# Start PostgreSQL and wait until it is healthy
db-up:
    docker compose up -d --wait postgres

# Stop local services without deleting database data
db-down:
    docker compose down

# Follow PostgreSQL logs
db-logs:
    docker compose logs --follow postgres

# Apply pending database migrations
migrate: db-up
    cargo sqlx migrate run

# Prepare the local database
setup: migrate

# Build the server
build:
    cargo build

# Start PostgreSQL, migrate, and run the server
run: migrate
    cargo run

# Run formatting, linting, and tests
check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test
