use crate::error::AppError;
use crate::game::game::{Game, GameId, Player, PlayerId};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, query_as};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

pub(crate) async fn establish_connection(db_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(50)
        .acquire_timeout(Duration::from_secs(3))
        .idle_timeout(Duration::from_secs(10))
        .connect(db_url)
        .await?;

    Ok(pool)
}

#[derive(sqlx::FromRow)]
pub(crate) struct GameRow {
    pub id: Uuid,
    pub status: String,
    pub state: serde_json::Value,
    pub maximum_players: i16,
    pub created_at: time::OffsetDateTime,
}

#[derive(sqlx::FromRow)]
pub(crate) struct GamePlayerRow {
    pub game_id: Uuid,
    pub id: Uuid,
    pub name: String,
    pub is_host: bool,
}

/// Games and their rosters are read separately, so they are stitched back
/// together here rather than in a `TryFrom` on a single row.
fn build_game(row: GameRow, player_rows: Vec<GamePlayerRow>) -> Result<Game, AppError> {
    let host = player_rows
        .iter()
        .find(|player| player.is_host)
        .map(|player| PlayerId::from_uuid(player.id))
        .ok_or_else(|| AppError::invalid_data("game has no host"))?;

    let players = player_rows
        .into_iter()
        .map(|player| Player {
            id: PlayerId::from_uuid(player.id),
            name: player.name,
        })
        .collect();

    Game::restore(
        GameId::from_uuid(row.id),
        row.status.parse()?,
        host,
        players,
        serde_json::from_value(row.state).map_err(AppError::invalid_data)?,
        u8::try_from(row.maximum_players).map_err(AppError::invalid_data)?,
        row.created_at,
    )
}

#[derive(sqlx::FromRow)]
pub(crate) struct PlayerRow {
    pub id: uuid::Uuid,
    pub name: String,
}
impl From<PlayerRow> for Player {
    fn from(row: PlayerRow) -> Self {
        Self {
            id: PlayerId::from_uuid(row.id),
            name: row.name,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct GameEvents {
    pub id: uuid::Uuid,
    pub kind: String,
}

pub(crate) async fn find_games(pool: &PgPool) -> Result<Vec<Game>, AppError> {
    let game_rows = query_as!(
        GameRow,
        r#"
        SELECT id, status, state, maximum_players, created_at
        FROM games
        "#
    )
    .fetch_all(pool)
    .await?;

    // One roster query for every game, rather than one per game.
    let game_ids: Vec<Uuid> = game_rows.iter().map(|row| row.id).collect();
    let player_rows = query_as!(
        GamePlayerRow,
        r#"
        SELECT gp.game_id, p.id, p.name, gp.is_host
        FROM game_players gp
        JOIN players p ON p.id = gp.player_id
        WHERE gp.game_id = ANY($1::uuid[])
        ORDER BY p.name
        "#,
        &game_ids
    )
    .fetch_all(pool)
    .await?;

    let mut rosters: HashMap<Uuid, Vec<GamePlayerRow>> = HashMap::new();
    for player_row in player_rows {
        rosters
            .entry(player_row.game_id)
            .or_default()
            .push(player_row);
    }

    game_rows
        .into_iter()
        .map(|row| {
            let roster = rosters.remove(&row.id).unwrap_or_default();
            build_game(row, roster)
        })
        .collect()
}

pub(crate) async fn find_game(pool: &PgPool, id: GameId) -> Result<Game, AppError> {
    let game_row = query_as!(
        GameRow,
        r#"
        SELECT id, status, state, maximum_players, created_at
        FROM games
        WHERE id = $1
        "#,
        id.to_uuid()
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let player_rows = query_as!(
        GamePlayerRow,
        r#"
        SELECT gp.game_id, p.id, p.name, gp.is_host
        FROM game_players gp
        JOIN players p ON p.id = gp.player_id
        WHERE gp.game_id = $1
        ORDER BY p.name
        "#,
        id.to_uuid()
    )
    .fetch_all(pool)
    .await?;

    build_game(game_row, player_rows)
}

pub(crate) async fn insert_game(pool: &PgPool, game: &Game) -> Result<(), AppError> {
    let state = serde_json::to_value(&game.state).map_err(AppError::unexpected)?;
    let player_ids: Vec<Uuid> = game
        .players
        .iter()
        .map(|player| player.id.to_uuid())
        .collect();

    // A game without its roster is not a valid game, so both writes land together.
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"
        INSERT INTO games (
            id,
            status,
            state,
            maximum_players,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5)
        "#,
        game.id.to_uuid(),
        game.status.as_str(),
        state,
        i16::from(game.max_players),
        game.created_at,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO game_players (game_id, player_id, is_host)
        SELECT $1, player_id, player_id = $3
        FROM UNNEST($2::uuid[]) AS player_id
        "#,
        game.id.to_uuid(),
        &player_ids,
        game.host.to_uuid(),
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

pub(crate) async fn insert_player(pool: &PgPool, player: &Player) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO players (id, name)
        VALUES ($1, $2)
        "#,
        player.id.to_uuid(),
        player.name.as_str(),
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn find_player(pool: &PgPool, player_id: PlayerId) -> Result<Player, AppError> {
    let row = query_as!(
        PlayerRow,
        r#"
        SELECT id, name
        FROM players
        WHERE id = $1
        "#,
        player_id.to_uuid()
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(row.into())
}

/// Looks the player up *within* a game, so a player who exists but has not
/// joined this game is a miss rather than a hit.
pub(crate) async fn find_game_player(
    pool: &PgPool,
    game_id: GameId,
    player_id: PlayerId,
) -> Result<Player, AppError> {
    let row = query_as!(
        PlayerRow,
        r#"
        SELECT p.id, p.name
        FROM game_players gp
        JOIN players p ON p.id = gp.player_id
        WHERE gp.game_id = $1 AND gp.player_id = $2
        "#,
        game_id.to_uuid(),
        player_id.to_uuid()
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(row.into())
}
