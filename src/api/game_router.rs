use crate::api::dto::{ApiPostResult, ApiResult, CreateGameRequest, GameDto, PlayerDto};
use crate::db;
use crate::game::game::{Game, GameId, PlayerId};
use crate::game::game_state::GameState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use time::OffsetDateTime;

#[derive(Deserialize)]
struct GameFilter {
    status: Option<u32>,
}

async fn post_games(
    State(pool): State<PgPool>,
    Json(request): Json<CreateGameRequest>,
) -> ApiPostResult<GameId> {
    let host = db::find_player(&pool, request.host).await?;
    let initiated_game = Game::initiate(request.maximum_players, host)?;
    db::insert_game(&pool, &initiated_game).await?;

    Ok((StatusCode::CREATED, Json(initiated_game.id)))
}
async fn get_games(
    State(pool): State<PgPool>,
    Query(filter): Query<GameFilter>,
) -> ApiResult<Vec<GameDto>> {
    let games = db::find_games(&pool).await?;

    Ok(Json(
        games
            .iter()
            .map(|game| GameDto {
                id: game.id,
                status: game.status,
            })
            .collect(),
    ))
}

async fn get_game(State(pool): State<PgPool>, Path(id): Path<GameId>) -> ApiResult<GameDto> {
    let game = db::find_game(&pool, id).await?;

    Ok(Json(game.into()))
}

async fn get_game_state(
    State(pool): State<PgPool>,
    Path(id): Path<GameId>,
) -> ApiResult<GameState> {
    let mut game = db::find_game(&pool, id).await?;
    let game_state = game.state_at(OffsetDateTime::now_utc())?.clone();

    Ok(Json(game_state))
}

async fn get_game_player(
    State(pool): State<PgPool>,
    Path((game_id, player_id)): Path<(GameId, PlayerId)>,
) -> ApiResult<PlayerDto> {
    let player = db::find_game_player(&pool, game_id, player_id).await?;

    Ok(Json(player.into()))
}

async fn post_game_start(
    State(pool): State<PgPool>,
    Path(id): Path<GameId>,
) -> ApiPostResult<GameDto> {
    let game = db::start_game(&pool, id).await?;

    Ok((StatusCode::OK, Json(game.into())))
}

pub(crate) fn create_game_router() -> Router<PgPool> {
    Router::new()
        .route("/", get(get_games).post(post_games))
        .route("/{id}", get(get_game))
        .route("/{id}/state", get(get_game_state))
        .route("/{id}/game/start", post(post_game_start))
        .route("/{game_id}/players/{player_id}", get(get_game_player))
}
