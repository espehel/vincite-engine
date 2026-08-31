use crate::api::dto::{CreateGameRequest, CreateGameResponse};
use crate::db;
use crate::error::AppError;
use crate::game::game::Game;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
struct GameFilter {
    status: Option<u32>,
}

async fn post_games(
    State(pool): State<PgPool>,
    Json(request): Json<CreateGameRequest>,
) -> Result<(StatusCode, Json<CreateGameResponse>), AppError> {
    let initiated_game = Game::initiate(request.maximum_players)?;
    db::insert_game(&pool, &initiated_game).await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateGameResponse {
            id: initiated_game.id,
        }),
    ))
}
async fn get_games(Query(filter): Query<GameFilter>) -> String {
    format!("fetching games with status {:?}", filter.status)
}

async fn get_game(Path(id): Path<Uuid>) -> String {
    format!("fetching game {id}")
}

async fn get_game_player(Path((game_id, player_id)): Path<(Uuid, Uuid)>) -> String {
    format!("fetching player {player_id} from {game_id}")
}

pub(crate) fn create_games_router() -> Router<PgPool> {
    Router::new()
        .route("/", get(get_games).post(post_games))
        .route("/{id}", get(get_game))
        .route("/{game_id}/players/{player_id}", get(get_game_player))
}
