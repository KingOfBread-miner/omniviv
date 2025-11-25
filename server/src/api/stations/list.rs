use crate::{api::AppState, models::OsmTramStation};
use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/api/stations",
    responses(
        (status = 200, description = "List of all tram stations from OpenStreetMap", body = Vec<OsmTramStation>)
    ),
    tag = "stations"
)]
pub async fn get_stations(State(state): State<AppState>) -> Response {
    let mut response = Json(state.stations.as_ref().clone()).into_response();

    response
}
