use crate::db::row_structs::{GameEventRow, GamePlayerRow, GameRow, PlayerRow};
use crate::error::AppError;
use crate::game::event::{GameEvent, NewGameEvent};
use crate::game::game::{Game, GameId, Player, PlayerId};
use sqlx::{PgConnection, PgExecutor, PgPool, query, query_as};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

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
        serde_json::from_value(row.state).ok(),
        u8::try_from(row.maximum_players).map_err(AppError::invalid_data)?,
        row.created_at,
        row.started_at,
    )
}

pub(crate) async fn find_games(pool: &PgPool) -> Result<Vec<Game>, AppError> {
    let game_rows = query_as!(
        GameRow,
        r#"
        SELECT id, status, state, maximum_players, created_at, started_at
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
        SELECT id, status, state, maximum_players, created_at, started_at
        FROM games
        WHERE id = $1
        "#,
        id.to_uuid()
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound {
        message: "game not found".to_string(),
    })?;

    let player_rows = fetch_roster(pool, id).await?;

    build_game(game_row, player_rows)
}

pub(crate) async fn fetch_roster(
    executor: impl PgExecutor<'_>,
    game_id: GameId,
) -> Result<Vec<GamePlayerRow>, AppError> {
    let rows = query_as!(
        GamePlayerRow,
        r#"
        SELECT gp.game_id, p.id, p.name, gp.is_host
        FROM game_players gp
        JOIN players p ON p.id = gp.player_id
        WHERE gp.game_id = $1
        ORDER BY p.name
        "#,
        game_id.to_uuid()
    )
    .fetch_all(executor)
    .await?;

    Ok(rows)
}

/// Loads a game and locks its row until the surrounding transaction ends, so
/// every command against the same game runs one at a time.
pub(crate) async fn lock_game(conn: &mut PgConnection, game_id: GameId) -> Result<Game, AppError> {
    let game_row = query_as!(
        GameRow,
        r#"SELECT id, status, state, maximum_players, created_at, started_at
           FROM games WHERE id = $1
           FOR UPDATE "#,
        game_id.to_uuid()
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(AppError::NotFound {
        message: "Game not found".to_owned(),
    })?;

    let player_rows = fetch_roster(&mut *conn, game_id).await?;

    build_game(game_row, player_rows)
}

/// Writes back the parts of a game a command can change. The roster lives in
/// `game_players` and is written by the command that changes it.
pub(crate) async fn update_game(conn: &mut PgConnection, game: &Game) -> Result<(), AppError> {
    query!(
        r#"UPDATE games SET status = $2, state = $3, started_at = $4 WHERE id = $1"#,
        game.id.to_uuid(),
        game.status.as_str(),
        serde_json::to_value(game.persisted_state()).map_err(AppError::unexpected)?,
        game.started_at
    )
    .execute(conn)
    .await?;

    Ok(())
}

pub(crate) async fn insert_game(pool: &PgPool, game: &Game) -> Result<(), AppError> {
    let state = serde_json::to_value(&game.persisted_state()).map_err(AppError::unexpected)?;
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
    query!(
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

pub(crate) async fn find_player(
    executor: impl PgExecutor<'_>,
    player_id: PlayerId,
) -> Result<Player, AppError> {
    let row = query_as!(
        PlayerRow,
        r#"
        SELECT id, name
        FROM players
        WHERE id = $1
        "#,
        player_id.to_uuid()
    )
    .fetch_optional(executor)
    .await?
    .ok_or(AppError::NotFound {
        message: "Player not found".to_string(),
    })?;

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
    .ok_or(AppError::NotFound {
        message: "Player does not exist".to_string(),
    })?;

    Ok(row.into())
}

pub(crate) async fn join_game(
    pool: &PgPool,
    game_id: GameId,
    player_id: PlayerId,
) -> Result<Game, AppError> {
    let mut tx = pool.begin().await?;

    let mut game = lock_game(&mut tx, game_id).await?;
    let player = find_player(&mut *tx, player_id).await?;
    game.join(player)?;

    query!(
        r#"
        INSERT INTO game_players (game_id, player_id, is_host)
        VALUES ($1, $2, FALSE)
        "#,
        game_id.to_uuid(),
        player_id.to_uuid(),
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(game)
}

pub(crate) async fn update_game_events(
    conn: &mut PgConnection,
    game_id: GameId,
    events: &[GameEvent],
) -> Result<(), AppError> {
    if events.is_empty() {
        return Ok(());
    }

    let event_ids = events.iter().map(|e| e.id).collect::<Vec<i64>>();
    let processed_ats = events
        .iter()
        .map(|e| e.processed_at)
        .collect::<Vec<Option<OffsetDateTime>>>();

    let result = sqlx::query(
        r#"
            UPDATE game_events AS e
            SET processed_at = batch.processed_at
            FROM UNNEST(
                $2::bigint[],
                $3::timestamptz[]
            ) AS batch(id, processed_at)
            WHERE e.game_id = $1
              AND e.id = batch.id
            "#,
    )
    .bind(game_id.to_uuid())
    .bind(&event_ids)
    .bind(&processed_ats)
    .execute(&mut *conn)
    .await?;

    if result.rows_affected() != event_ids.len() as u64 {
        return Err(AppError::invalid_data(
            "some events were not found in their games",
        ));
    }
    Ok(())
}

pub(crate) async fn insert_game_events(
    conn: &mut PgConnection,
    game_id: GameId,
    events: &[NewGameEvent],
) -> Result<Vec<GameEvent>, AppError> {
    if events.is_empty() {
        return Ok(Vec::new());
    }

    let command_ids: Vec<_> = events.iter().map(|e| e.command_id).collect();
    let due_ticks: Vec<i64> = events.iter().map(|e| e.due_tick).collect();
    let due_ats: Vec<_> = events.iter().map(|e| e.due_at).collect();
    let data: Vec<_> = events
        .iter()
        .map(|e| serde_json::to_value(&e.data))
        .collect::<Result<_, _>>()
        .map_err(AppError::unexpected)?;

    let kinds: Vec<String> = data
        .iter()
        .map(|d| {
            d.get("kind")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| AppError::invalid_data("missing event kind"))
        })
        .collect::<Result<_, _>>()?;

    let payloads: Vec<_> = data
        .iter()
        .map(|d| {
            d.get("payload")
                .cloned()
                .ok_or_else(|| AppError::invalid_data("missing event payload"))
        })
        .collect::<Result<_, _>>()?;

    let rows = sqlx::query_as::<_, GameEventRow>(
        r#"
        INSERT INTO game_events (
            game_id, command_id, kind, payload, due_tick, due_at
        )
        SELECT $1, batch.*
        FROM UNNEST(
            $2::bigint[],
            $3::text[],
            $4::jsonb[],
            $5::bigint[],
            $6::timestamptz[]
        ) AS batch(command_id, kind, payload, due_tick, due_at)
        RETURNING *
        "#,
    )
    .bind(game_id.to_uuid())
    .bind(&command_ids)
    .bind(&kinds)
    .bind(&payloads)
    .bind(&due_ticks)
    .bind(&due_ats)
    .fetch_all(&mut *conn)
    .await?;

    rows.iter().map(|row| row.clone().into()).collect()
}
