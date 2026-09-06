use crate::api::dto::{ApiPostResult, ApiResult, CreatePlayerRequest, PlayerDto};
use crate::db;
use crate::game::game::{Player, PlayerId};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use sqlx::PgPool;

async fn post_players(
    State(pool): State<PgPool>,
    Json(request): Json<CreatePlayerRequest>,
) -> ApiPostResult<PlayerId> {
    let new_player = Player::new(request.name);
    db::insert_player(&pool, &new_player).await?;

    Ok((StatusCode::CREATED, Json(new_player.id)))
}

async fn get_player(State(pool): State<PgPool>, Path(id): Path<PlayerId>) -> ApiResult<PlayerDto> {
    let player = db::find_player(&pool, id).await?;

    Ok(Json(player.into()))
}

pub(crate) fn create_player_router() -> Router<PgPool> {
    Router::new()
        .route("/", post(post_players))
        .route("/{id}", get(get_player))
}
