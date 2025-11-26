use crate::api::AppState;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/api/stations",
    responses(
        (status = 200, description = "Map of all tram stations with platforms from OSM (keyed by station_id, platforms include ref:IFOPT tags)")
    ),
    tag = "stations"
)]
pub async fn get_stations(State(state): State<AppState>) -> Response {
    let response = Json(state.stations.as_ref().clone()).into_response();

    response
}
