use crate::{api::AppState, models::TramLine};
use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/api/lines",
    responses(
        (status = 200, description = "List of all tram lines from OpenStreetMap", body = Vec<TramLine>)
    ),
    tag = "lines"
)]
pub async fn get_lines(State(state): State<AppState>) -> Response {
    let mut response = Json(state.lines.as_ref().clone()).into_response();

    // Cache for 24 hours - lines are static data that rarely changes
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "public, max-age=86400".parse().unwrap(),
    );

    response
}
