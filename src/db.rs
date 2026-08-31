use crate::error::AppError;
use crate::game::game::{Game, GameId};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Error, PgPool, query_as};
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
impl TryFrom<GameRow> for Game {
    type Error = AppError;

    fn try_from(row: GameRow) -> Result<Self, Self::Error> {
        Game::restore(
            GameId::from_uuid(row.id),
            row.status.parse()?,
            serde_json::from_value(row.state).map_err(AppError::invalid_data)?,
            u8::try_from(row.maximum_players).map_err(AppError::invalid_data)?,
            row.created_at,
        )
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct GamePlayerRow {
    pub id: uuid::Uuid,
    pub name: String,
}

#[derive(sqlx::FromRow)]
pub(crate) struct GameEvents {
    pub id: uuid::Uuid,
    pub kind: String,
}

pub(crate) async fn findGame(pool: &PgPool, id: Uuid) -> Result<Option<GameRow>, Error> {
    let row = query_as!(
        GameRow,
        r#"SELECT id, status, state, maximum_players, created_at FROM games WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub(crate) async fn insert_game(pool: &PgPool, game: &Game) -> Result<(), AppError> {
    let state = serde_json::to_value(&game.state).map_err(AppError::unexpected)?;

    let result = sqlx::query!(
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
    .execute(pool)
    .await?;

    Ok(())
}
