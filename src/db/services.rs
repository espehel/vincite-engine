use crate::db::queries::{insert_game_events, lock_game, update_game, update_game_events};
use crate::error::AppError;
use crate::game::command::GameCommand;
use crate::game::event::GameEvent;
use crate::game::game::{Game, GameId, PlayerId};
use sqlx::PgPool;
use time::OffsetDateTime;

pub(crate) async fn start_game(pool: &PgPool, game_id: GameId) -> Result<Game, AppError> {
    let mut tx = pool.begin().await?;
    let mut game = lock_game(&mut tx, game_id).await?;

    game.start()?;
    update_game(&mut tx, &game).await?;

    tx.commit().await?;
    Ok(game)
}

pub(crate) async fn apply_command(
    pool: &PgPool,
    game_id: GameId,
    player_id: PlayerId,
    command: GameCommand,
) -> Result<Vec<GameEvent>, AppError> {
    let now = OffsetDateTime::now_utc();
    let mut tx = pool.begin().await?;
    let mut game = lock_game(&mut tx, game_id).await?;

    let processedEvents = game.advanceTo(now)?;
    let new_events = game.apply_command(player_id, command)?;

    update_game_events(&mut tx, game_id, &processedEvents).await?;
    let inserted_events = insert_game_events(&mut tx, game_id, &new_events).await?;
    tx.commit().await?;
    Ok(inserted_events)
}
