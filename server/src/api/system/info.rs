use crate::api::AppState;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EfaApiMetrics {
    /// Total number of requests made to EFA API
    pub total_requests: u64,
    /// Number of requests in the last second
    pub requests_last_second: u64,
    /// Number of requests in the last minute
    pub requests_last_minute: u64,
    /// Average requests per second over the last minute
    pub avg_rps_last_minute: f64,
    /// Current requests per second (based on last second)
    pub current_rps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SystemInfo {
    /// EFA API request metrics
    pub efa_api_metrics: EfaApiMetrics,
    /// Number of stations from OSM
    pub total_stations: usize,
    /// Number of stations with ref:IFOPT tags
    pub stations_with_ifopt: usize,
    /// Number of cached stop event responses
    pub cached_stop_events: usize,
    /// Number of tracked vehicles
    pub tracked_vehicles: usize,
    /// Cache update interval in seconds
    pub cache_update_interval_seconds: u64,
    /// System uptime information
    pub server_version: String,
    /// Timestamp when this info was generated
    pub timestamp: String,
}

#[utoipa::path(
    get,
    path = "/api/system/info",
    responses(
        (status = 200, description = "System information and metrics", body = SystemInfo)
    ),
    tag = "system"
)]
pub async fn get_system_info(State(state): State<AppState>) -> Response {
    // Get EFA metrics
    let metrics = state.efa_metrics.get_metrics().await;

    // Count stations from OSM
    let total_stations = state.stations.len();
    let stations_with_ifopt = state
        .stations
        .values()
        .flat_map(|station| &station.platforms)
        .filter(|platform| platform.osm_tags.as_ref()
            .and_then(|tags| tags.get("ref:IFOPT"))
            .is_some())
        .count();

    // Count cached stop events
    let cached_stop_events = match state.stop_events.read() {
        Ok(cache) => cache.len(),
        Err(_) => 0,
    };

    // Count tracked vehicles (from vehicle list, not positions)
    let tracked_vehicles = match state.vehicles.read() {
        Ok(cache) => cache.len(), // Only count active vehicles
        Err(_) => 0,
    };

    let system_info = SystemInfo {
        efa_api_metrics: EfaApiMetrics {
            total_requests: metrics.total_requests,
            requests_last_second: metrics.requests_last_second,
            requests_last_minute: metrics.requests_last_minute,
            avg_rps_last_minute: metrics.avg_rps_last_minute,
            current_rps: metrics.current_rps,
        },
        total_stations,
        stations_with_ifopt,
        cached_stop_events,
        tracked_vehicles,
        cache_update_interval_seconds: 5,
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(system_info).into_response()
}
