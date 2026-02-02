use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::sync::ScheduleStore;

#[derive(Clone)]
pub struct HealthState {
    pub schedule_store: ScheduleStore,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Whether the service is running
    pub healthy: bool,
    /// Whether the static GTFS schedule has been loaded into memory
    pub gtfs_schedule_loaded: bool,
    /// Number of GTFS stops in the loaded schedule
    pub gtfs_stop_count: usize,
    /// Number of GTFS routes in the loaded schedule
    pub gtfs_route_count: usize,
    /// Number of GTFS trips in the loaded schedule
    pub gtfs_trip_count: usize,
    /// Number of IFOPT-to-GTFS stop mappings
    pub ifopt_mapping_count: usize,
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Service health status", body = HealthResponse)
    ),
    tag = "health"
)]
pub async fn health_check(State(state): State<HealthState>) -> Json<HealthResponse> {
    let schedule_guard = state.schedule_store.read().await;
    let (loaded, stop_count, route_count, trip_count, ifopt_count) =
        if let Some(schedule) = schedule_guard.as_ref() {
            (
                true,
                schedule.stops.len(),
                schedule.routes.len(),
                schedule.trips.len(),
                schedule.ifopt_to_gtfs.len(),
            )
        } else {
            (false, 0, 0, 0, 0)
        };

    Json(HealthResponse {
        healthy: true,
        gtfs_schedule_loaded: loaded,
        gtfs_stop_count: stop_count,
        gtfs_route_count: route_count,
        gtfs_trip_count: trip_count,
        ifopt_mapping_count: ifopt_count,
    })
}

pub fn router(schedule_store: ScheduleStore) -> Router {
    let state = HealthState { schedule_store };
    Router::new()
        .route("/", get(health_check))
        .with_state(state)
}
