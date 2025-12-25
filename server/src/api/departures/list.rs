use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::ErrorResponse;
use crate::sync::{Departure, DepartureStore};

/// Filter out departures that are in the past
fn filter_past_departures(departures: Vec<Departure>) -> Vec<Departure> {
    let now = Utc::now();
    departures
        .into_iter()
        .filter(|d| {
            // Use estimated time if available, otherwise planned time
            let time_str = d.estimated_time.as_ref().unwrap_or(&d.planned_time);
            match chrono::DateTime::parse_from_rfc3339(time_str) {
                Ok(time) => time > now,
                Err(_) => true, // Keep if we can't parse the time
            }
        })
        .collect()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DepartureListResponse {
    pub departures: Vec<Departure>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StopDeparturesRequest {
    pub stop_ifopt: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StopDeparturesResponse {
    pub stop_ifopt: String,
    pub departures: Vec<Departure>,
}

/// List all departures across all stops
#[utoipa::path(
    get,
    path = "/api/departures",
    responses(
        (status = 200, description = "List of all departures", body = DepartureListResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "departures"
)]
pub async fn list_departures(
    State(store): State<DepartureStore>,
) -> Json<DepartureListResponse> {
    let store = store.read().await;
    let departures: Vec<Departure> = store.values().flatten().cloned().collect();
    let departures = filter_past_departures(departures);
    Json(DepartureListResponse { departures })
}

/// Get departures for a specific stop by IFOPT ID
#[utoipa::path(
    post,
    path = "/api/departures/by-stop",
    request_body = StopDeparturesRequest,
    responses(
        (status = 200, description = "Departures for the stop", body = StopDeparturesResponse),
        (status = 400, description = "Bad request", body = ErrorResponse)
    ),
    tag = "departures"
)]
pub async fn get_departures_by_stop(
    State(store): State<DepartureStore>,
    Json(request): Json<StopDeparturesRequest>,
) -> Json<StopDeparturesResponse> {
    let store = store.read().await;
    let departures = store.get(&request.stop_ifopt).cloned().unwrap_or_default();
    let departures = filter_past_departures(departures);

    Json(StopDeparturesResponse {
        stop_ifopt: request.stop_ifopt,
        departures,
    })
}
