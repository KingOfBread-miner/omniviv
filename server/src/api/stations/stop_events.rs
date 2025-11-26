use crate::api::AppState;
use crate::services::efa::EfaDepartureMonitorResponse;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/api/stations/stop_events/{platform_id}",
    params(
        ("platform_id" = String, Path, description = "Platform ID (IFOPT reference, e.g., de:09761:101:0:A)")
    ),
    responses(
        (status = 200, description = "Real-time stop events for the specified platform (cached, updated every 5 seconds)", body = EfaDepartureMonitorResponse),
        (status = 404, description = "Platform not found or no stop events available")
    ),
    tag = "stations"
)]
pub async fn get_stop_events(
    State(state): State<AppState>,
    Path(platform_id): Path<String>,
) -> Response {
    // Extract station_id from platform_id (first 3 parts of IFOPT)
    // e.g., "de:09761:101:0:A" -> "de:09761:101"
    let parts: Vec<&str> = platform_id.split(':').collect();
    let station_id = if parts.len() >= 3 {
        format!("{}:{}:{}", parts[0], parts[1], parts[2])
    } else {
        // If not a valid IFOPT, try using the whole ID as station_id
        platform_id.clone()
    };

    // Acquire read lock on the cache
    let cache = match state.stop_events.read() {
        Ok(cache) => cache,
        Err(e) => {
            tracing::error!(error = %e, "Failed to acquire read lock on stop events cache");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read stop events cache",
            )
                .into_response();
        }
    };

    // Get stop events for the station
    match cache.get(&station_id) {
        Some(station_events) => {
            // Filter stop events to only those for the requested platform
            let mut filtered_response = station_events.clone();
            filtered_response.stop_events = station_events
                .stop_events
                .iter()
                .filter(|event| event.location.id == platform_id)
                .cloned()
                .collect();

            Json(filtered_response).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            format!("No stop events found for platform: {}", platform_id),
        )
            .into_response(),
    }
}
