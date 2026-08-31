# Project structure

Rust does not prescribe a Rails-style directory structure. Cargo defines crates, packages, modules, and workspaces, while application layering is your architectural choice. [Rust's module documentation](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)

For this game server the guiding principle is:

> **Keep the game rules pure. Everything else is plumbing, and plumbing should be short.**

A layered hexagonal architecture (domain / application / infrastructure, repository traits, three models per concept) is a good fit for a large system maintained by a team. At this size it costs several hundred lines of conversion code and buys nothing yet. This document describes the smaller arrangement to start from, and records the point at which each piece of structure earns its place.

## Recommended structure

```text
vincite/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── justfile
├── compose.yaml
├── .env.example
├── .sqlx/                     committed query cache, so `cargo check` works without a database
├── migrations/
├── src/
│   ├── main.rs                config, pool, migrations, router, ticker, serve
│   ├── error.rs               one AppError + IntoResponse
│   ├── db.rs                  every SQL query, using sqlx directly
│   ├── ticker.rs              the engine loop
│   ├── ws.rs                  websocket upgrade + per-game broadcast
│   ├── api/
│   │   ├── mod.rs
│   │   ├── router.rs          thin handlers
│   │   └── dto.rs             request/response types
│   └── game/
│       ├── mod.rs             Game, GameState, GameStatus, GameId, PlayerId
│       ├── command.rs         Command, Event
│       └── rules.rs           pure functions — the part that matters
└── tests/
    ├── create_game_api.rs
    └── tick.rs
```

Nine files rather than twenty-five. The only boundary enforced from day one is the one that pays for itself immediately:

```text
        api/  ──┐
        ws.rs ──┼──►  db.rs  ──►  Postgres
     ticker.rs ─┘       │
                        ▼
                     game/  ← pure, synchronous, deterministic, no sqlx, no axum
```

`game/` depends on nothing but `serde` and the standard library. Everything else may depend on it. That is the whole architecture.

## One type per concept, not three

The instinct to split each concept into a domain type, a row type, and a DTO triples the code and produces four error-conversion hops for a single `SELECT`. Do it when a representation actually diverges, not preemptively.

Because game state is stored as a single JSONB blob, one struct can serve all three roles:

```rust
// src/game/mod.rs

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct GameId(pub Uuid);

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "game_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    Open,
    Running,
    Closed,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GameState {
    // the entire simulation lives here
}

#[derive(sqlx::FromRow, Debug)]
pub struct Game {
    pub id: GameId,
    pub status: GameStatus,
    pub state: sqlx::types::Json<GameState>,
    pub maximum_players: i16,
    pub next_tick_at: OffsetDateTime,
}
```

Two derives replace code that would otherwise be written by hand:

- `#[sqlx(transparent)]` on `GameId` maps the newtype straight to `UUID`, removing `to_uuid`/`from_uuid` conversions at every call site.
- `#[sqlx(type_name = "game_status")]` maps the enum to a native Postgres enum, removing the hand-written `match` on `status.as_str()` and the "invalid data" error branch that went with it.

Both require a migration adding `CREATE TYPE game_status AS ENUM ('open', 'running', 'closed');`.

The cost is that `query_as!` needs explicit column type overrides. This is a fair trade for keeping compile-time query verification:

```rust
sqlx::query_as!(
    Game,
    r#"SELECT id      as "id: GameId",
              status  as "status: GameStatus",
              state   as "state: Json<GameState>",
              maximum_players,
              next_tick_at
       FROM games WHERE id = $1"#,
    id.0
)
```

**Introduce a separate DTO the first time an API field diverges from a column** — a computed `player_count`, a field you want to hide, a rename. Not before. The rule that matters is the one about direction: never serialize a database row straight to a client, or a migration silently becomes an API-breaking change.

## No repository trait

A `GameRepository` trait with a single `PostgresGameRepository` implementation is not an abstraction, it is indirection. It is also a trap in current Rust:

```rust
pub trait GameRepository: Send + Sync {
    async fn find(&self, id: GameId) -> Result<Option<Game>, RepositoryError>;
}
```

`async fn` in a trait is **not dyn-compatible**. `Arc<dyn GameRepository>` will not compile. Every consumer must therefore be generic over `<R: GameRepository>`, that parameter propagates into application services, into `AppState`, and into handler signatures, and because the returned futures are not guaranteed `Send`, generic code passed to `tokio::spawn` fails to compile. Escaping this means `async-trait` (which boxes and allocates on every call) or `trait_variant`.

None of that buys anything while there is one implementation. Write free functions taking `&PgPool`:

```rust
// src/db.rs
pub(crate) async fn find_game(pool: &PgPool, id: GameId) -> Result<Option<Game>, sqlx::Error>
pub(crate) async fn insert_game(pool: &PgPool, maximum_players: u8) -> Result<GameId, sqlx::Error>
pub(crate) async fn due_games(tx: &mut PgTransaction<'_>) -> Result<Vec<Game>, sqlx::Error>
```

Take `&PgPool`, never `PgPool` by value — passing by value forces a clone at every call site for no reason.

**Add a trait when there is a second implementation you actually need** — most likely an in-memory store for tests. Even then, prefer testing against a real Postgres in a transaction that rolls back; it catches SQL errors a fake never will.

## Wiring: how `POST /games` reaches the database

This is the mechanism that replaces dependency injection, and it is worth understanding precisely.

```text
main()
  │
  ├─ establish_connection()  ──►  PgPool          ← Arc<PoolInner>, owns ≤50 connections
  │                                  │
  ├─ Router::new()                   │
  │    .nest("/games", …)     Router<PgPool>      ← "I still need a PgPool"
  │    .with_state(pool)  ────────►  Router<()>   ← "fully supplied, ready to serve"
  │                                  │
  └─ axum::serve(listener, app)      │
                                     ▼
        POST /games  ──►  post_games(State(pool): State<PgPool>, Json(req))
                                     │            ← a clone: same Arc, same 50 connections
                                     ▼
                          db::insert_game(&pool, …)
                                     │
                                     ▼
                          sqlx …execute(pool)
                                     │            ← acquire a connection, INSERT, release
                                     ▼
                                 Postgres
```

Two facts make this work.

**`PgPool` is already an `Arc`.** `sqlx::Pool` is a newtype over `Arc<PoolInner>`; cloning bumps a refcount and every clone shares one connection set. Never wrap it in `Arc<PgPool>` — that is a pointer to a pointer. Axum clones the state once per request, and "the right pool" is automatic because only one exists.

**`Router<S>` tracks the state still owed.** A bare `Router` means `Router<()>` — nothing outstanding. A router whose handlers ask for `State<PgPool>` is a `Router<PgPool>`. `with_state` discharges the obligation and returns `Router<()>`, which is the only thing `axum::serve` accepts. Getting this wrong produces type errors that never mention state.

```rust
// src/main.rs
let pool = establish_connection(&database_url).await?;
sqlx::migrate!().run(&pool).await?;

let app = Router::new()
    .nest("/games", api::create_games_router())  // Router<PgPool>
    .with_state(pool.clone());                   // -> Router<()>

tokio::spawn(ticker::run(pool));
axum::serve(listener, app).await?;
```

```rust
// src/api/router.rs
pub(crate) fn create_games_router() -> Router<PgPool> { … }   // not `Router`

async fn post_games(
    State(pool): State<PgPool>,               // FromRequestParts
    Json(request): Json<CreateGameRequest>,   // FromRequest — consumes the body, MUST be last
) -> Result<(StatusCode, Json<CreateGameResponse>), AppError> {
    let id = db::insert_game(&pool, request.maximum_players).await?;
    Ok((StatusCode::CREATED, Json(CreateGameResponse { id })))
}
```

Argument order is a rule, not a style preference. `Json` implements `FromRequest` and takes the body by value; every other extractor implements `FromRequestParts`. Only the final argument may be `FromRequest`.

Axum is deliberately modular and imposes no project layout, so thin handlers calling free functions fit it naturally. [Axum documentation](https://github.com/tokio-rs/axum)

### When state grows beyond the pool

The WebSocket broadcast handles will need to live alongside the pool. Do not rewrite every handler to take `State<AppState>` — implement `FromRef` so each handler keeps asking only for what it needs:

```rust
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub games: Arc<DashMap<GameId, broadcast::Sender<ServerMessage>>>,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self { state.pool.clone() }
}
```

`State<PgPool>` in `post_games` continues to work unchanged. Note that `AppState` derives `Clone` and holds an `Arc` for the map, for the same reason the pool is cheap to clone.

## One error type

Three parallel error enums (`GameError`, `RepositoryError`, `ApiError`) means converting errors more often than handling them. Start with a single `AppError` in `src/error.rs`, and split only when a caller genuinely needs to match on a distinction that `AppError` cannot express.

Two impls are what make `?` work inside a handler:

```rust
impl From<sqlx::Error> for AppError {          // lets `?` convert
    fn from(error: sqlx::Error) -> Self {
        Self::Unexpected { source: Box::new(error) }
    }
}

impl IntoResponse for AppError {               // lets axum return it
    fn into_response(self) -> Response {
        let status = match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::AlreadyExists | Self::Conflict => StatusCode::CONFLICT,
            Self::InvalidData { .. } => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
```

`AppError` is a valid handler return type only because of `IntoResponse`, and `?` compiles only because of `From`. `into_response` must be the trait implementation, not an inherent method.

Log the internal error before discarding it; never return `source` to a client.

## The engine loop

Regular state advancement does not need a manager, a runtime, and a per-game actor. Let the database decide what is due, and let row locks provide concurrency control:

```rust
// src/ticker.rs
async fn tick_due_games(pool: &PgPool) -> anyhow::Result<usize> {
    let mut tx = pool.begin().await?;

    let games = sqlx::query_as!(Game, r#"
        SELECT id as "id: GameId", status as "status: GameStatus",
               state as "state: Json<GameState>", maximum_players, next_tick_at
        FROM games
        WHERE status = 'running' AND next_tick_at <= now()
        ORDER BY next_tick_at
        LIMIT 100
        FOR UPDATE SKIP LOCKED"#)
        .fetch_all(&mut *tx)
        .await?;

    for game in &mut games {
        rules::tick(&mut game.state.0, TICK_INTERVAL);
        // UPDATE games SET state = $1, next_tick_at = next_tick_at + $2 WHERE id = $3
    }

    tx.commit().await?;
    Ok(games.len())
}
```

`FOR UPDATE SKIP LOCKED` does two jobs at once: it lets several server instances tick disjoint sets of games without coordination, and it prevents a tick from colliding with a player command. Player commands take `SELECT … WHERE id = $1 FOR UPDATE` in their own transaction, so the two serialize on the row.

**That is why there is no `version` column and no optimistic-concurrency retry loop.** One mechanism instead of two. Add a version column only if a use case appears that cannot hold a transaction open.

Drive the loop with `tokio::time::interval`, setting `MissedTickBehavior::Delay` so a slow batch does not cause a burst of catch-up iterations. `next_tick_at` in the database is authoritative for game time; the interval only controls how often the server looks.

## Realtime updates

Start with a `broadcast::Sender<ServerMessage>` per active game, held in the shared map described above. A handler mutating a game sends on the channel; each connected WebSocket task forwards to its client.

When a second server instance appears, upgrade to Postgres `LISTEN`/`NOTIFY` via `sqlx::postgres::PgListener`: the writer issues `NOTIFY game_updates, '<game_id>'`, and every instance fans out to its own local subscribers. Multi-instance realtime with no additional infrastructure.

## Persistence and event sourcing

Store the current state snapshot. Do not start with event sourcing — reconstructing state from an event log is the single largest complexity multiplier available, and it constrains every schema decision that follows.

An append-only `game_events` table is still worth having, but as a *derived* log for debugging, analytics, and replay investigation — not as the source of truth. It needs `game_id`, an ordering column, a payload, and a timestamp; a table of `(id, kind)` cannot serve any of those purposes.

Revisit this if and only if a requirement appears that a snapshot genuinely cannot satisfy, such as authoritative rewind or client-side deterministic replay.

## Schema notes

- `games.state` and `games.created_at` have no defaults, so every `INSERT` must supply them. Adding `DEFAULT '{}'::jsonb` and `DEFAULT now()` shortens every insert.
- `games` needs a `next_tick_at TIMESTAMPTZ` column with an index on `(status, next_tick_at)` for the ticker query to stay cheap.
- `game_players` needs a `game_id` foreign key; as written the table cannot associate a player with a game.
- The `maximum_players BETWEEN 0 AND 255` constraint permits zero-player games.
- Commit `.sqlx/` (`cargo sqlx prepare`) so `cargo check` and CI work without a live database.

## Rust module conventions

Prefer `game.rs` beside a `game/` directory over `game/mod.rs`, to avoid a tree full of identically named files:

```text
game.rs
game/
  command.rs
  rules.rs
```

```rust
// game.rs
mod command;
mod rules;

pub use command::{Command, Event};
```

Other conventions:

- Files and modules: `snake_case`. Types and traits: `PascalCase`. Functions: `snake_case`.
- Keep modules private by default; prefer `pub(crate)` over `pub` for internal APIs.
- Re-export only the types callers are meant to use.
- Avoid dumping grounds such as `utils.rs`, `common.rs`, or `helpers.rs`.
- Split a file when it holds more than one clear responsibility, not merely because it has grown long.

## Tests

Keep unit tests beside the code. `game/rules.rs` is the highest-value target precisely because it is pure — no database, no async, no fixtures:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_cannot_join_started_game() {
        // ...
    }
}
```

Use root-level `tests/` for tests crossing module boundaries:

```text
tests/
  create_game_api.rs
  tick.rs
```

Integration tests need `src/lib.rs` so the crate is importable. Add it at that point; until then a single `main.rs` binary is fine, and the `lib.rs` → `run()` → `app::run()` indirection adds nothing.

For the game rules, property-based tests with `proptest` are especially valuable for invariants such as resource totals, valid ownership, and deterministic replay.

## When to add structure back

Every piece of removed structure has a trigger that earns it back:

| Add | When |
| --- | --- |
| Separate DTO types | An API field diverges from a column |
| A repository trait | A second implementation is genuinely required |
| `application/` use-case types | A handler coordinates three or more operations across a transaction |
| `lib.rs` | The first integration test |
| A `version` column | A write path cannot hold a transaction open |
| `PgListener` fanout | A second server instance |
| Event sourcing | Authoritative rewind or client-side replay is required |
| A workspace | Module boundaries have been stable for months |

A later workspace would plausibly look like:

```text
crates/
  game/       Rules, commands, events — pure
  protocol/   Shared WebSocket message types, compiled to WASM for the client
  server/     Axum executable, sqlx, ticker
```

Cargo workspaces share a lockfile and build output while allowing packages to be tested independently. [Cargo workspace documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html)

The `protocol/` crate is the one with the strongest long-term case, because sharing message definitions with a WASM client eliminates an entire class of client/server drift. Extract crates when a boundary has already proven stable and painful, not to enforce a diagram.

---

The boundary that matters here is not "controller versus service". It is:

> **Deterministic game rules → everything else.**

One `rules.rs` of pure functions delivers that. No trait, no layers, no conversion code.
