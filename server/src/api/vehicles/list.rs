use std::collections::HashMap;

use crate::{api::AppState, models::{VehicleInfo, VehicleListResponse}};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Get list of all unique vehicles currently on the network
///
/// This endpoint returns a list of all unique vehicles with stale tracking.
/// Vehicles are marked as stale when they're not found in recent stop events,
/// and are removed after 5 minutes of being stale. This is useful for:
/// - Verifying the cache is populated
/// - Seeing which vehicles are currently active vs stale
/// - Debugging the vehicle tracking system
///
/// The list is served from an in-memory cache that's updated every 5 seconds
/// along with the stop events cache.
#[utoipa::path(
    get,
    path = "/api/vehicles/list",
    responses(
        (status = 200, description = "List of all unique vehicles with stale tracking", body = VehicleListResponse)
    ),
    tag = "vehicles"
)]
pub async fn get_vehicles_list(State(state): State<AppState>) -> Response {
    // Acquire read lock on the vehicle list cache
    let cache = match state.vehicles.read() {
        Ok(cache) => cache,
        Err(e) => {
            tracing::error!(error = %e, "Failed to acquire read lock on vehicle list cache");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read vehicle list cache",
            )
                .into_response();
        }
    };

    // Clone the data to release the lock quickly
    let vehicles: HashMap<String, VehicleInfo> = cache.clone();

    // Calculate counts
    let total_count = vehicles.len();
    let active_count = vehicles.values().filter(|v| !v.is_stale).count();
    let stale_count = vehicles.values().filter(|v| v.is_stale).count();

    let response = VehicleListResponse {
        vehicles,
        total_count,
        active_count,
        stale_count,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(response).into_response()
}
