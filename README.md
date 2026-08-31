## Prerequisites

- Rust installed through rustup
- Docker with Docker Compose
- just
- SQLx CLI

Install development tools:

    cargo install just
    cargo install sqlx-cli --no-default-features --features rustls,postgres

## Initial setup

    cp .env.example .env
    just setup

## Run the server

    just run

## Stop the database

    just db-down

Run `just` to list all available development commands.
