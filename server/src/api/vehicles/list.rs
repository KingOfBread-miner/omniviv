use std::collections::HashMap;

use crate::{api::AppState, models::{VehicleInfo, VehicleListResponse}};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Get list of all currently active vehicles on the network
///
/// This endpoint returns comprehensive information about all vehicles currently
/// operating in the tram system using a time-window based approach.
///
/// # Time-Window Filtering
/// Only vehicles with departures within the active time window are included:
/// - **Past**: -20 minutes (recently departed, still on route)
/// - **Future**: +20 minutes (about to depart)
/// - **Total window**: 40 minutes around current time
///
/// This ensures only currently active or soon-to-depart vehicles are shown.
///
/// # Vehicle Identification
/// - Each vehicle is identified by its `tripCode` (unique trip identifier)
/// - Physical vehicle ID stored separately when available
/// - One trip = one vehicle entry
///
/// # Information Provided
/// - Current location (platform IFOPT and name)
/// - Next stop and upcoming stops
/// - Previous stops (journey history)
/// - Departure times (planned and estimated)
/// - Delay information
/// - Line number, destination, and origin
///
/// # Expected Count
/// Should show ~40-60 vehicles during operational hours, matching the active fleet.
///
/// # Update Frequency
/// The vehicle list is recalculated every 5 seconds from the stop events cache (stateless).
#[utoipa::path(
    get,
    path = "/api/vehicles/list",
    responses(
        (status = 200, description = "Comprehensive list of all vehicles with location, timing, and journey context", body = VehicleListResponse)
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
