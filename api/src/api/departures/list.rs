use axum::{extract::State, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use utoipa::ToSchema;

use crate::api::ErrorResponse;
use crate::providers::timetables::gtfs::realtime;
use crate::sync::Departure;

use super::DeparturesState;

/// Filter out departures that are in the past relative to the given reference time
fn filter_past_departures(departures: Vec<Departure>, reference_time: DateTime<Utc>) -> Vec<Departure> {
    departures
        .into_iter()
        .filter(|d| {
            // Use estimated time if available, otherwise planned time
            let time_str = d.estimated_time.as_ref().unwrap_or(&d.planned_time);
            match chrono::DateTime::parse_from_rfc3339(time_str) {
                Ok(time) => time > reference_time,
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
    /// Optional reference time (ISO 8601/RFC 3339) for time simulation.
    /// When provided, departures are computed from the static GTFS schedule
    /// around this time instead of using live real-time data.
    pub reference_time: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StopDeparturesResponse {
    pub stop_ifopt: String,
    pub departures: Vec<Departure>,
}

/// Parse a reference_time string and determine if it's a simulated (non-current) time.
/// Returns Some(DateTime) if it's a valid future/past simulated time, None if it's effectively "now".
fn parse_reference_time(reference_time: &Option<String>) -> Option<DateTime<Utc>> {
    let rt = reference_time.as_ref()?;
    let parsed = DateTime::parse_from_rfc3339(rt).ok()?;
    let dt = parsed.with_timezone(&Utc);

    // If the reference time is within 3 minutes of now, treat it as real-time
    let diff = (dt - Utc::now()).num_seconds().abs();
    if diff < 180 {
        return None;
    }
    Some(dt)
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
    State(state): State<DeparturesState>,
) -> Json<DepartureListResponse> {
    let store = state.departure_store.read().await;
    let departures: Vec<Departure> = store.values().flatten().cloned().collect();
    let departures = filter_past_departures(departures, Utc::now());
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
    State(state): State<DeparturesState>,
    Json(request): Json<StopDeparturesRequest>,
) -> Json<StopDeparturesResponse> {
    let simulated_time = parse_reference_time(&request.reference_time);

    let departures = if let Some(ref_time) = simulated_time {
        // Compute departures from static schedule for the simulated time
        let schedule_guard = state.schedule_store.read().await;
        if let Some(schedule) = schedule_guard.as_ref() {
            let mut stop_ids = HashSet::new();
            stop_ids.insert(request.stop_ifopt.clone());
            let time_horizon = Duration::minutes(state.time_horizon_minutes as i64);
            let all_departures = realtime::compute_schedule_departures(
                schedule,
                &stop_ids,
                ref_time,
                time_horizon,
                state.timezone,
            );
            all_departures.get(&request.stop_ifopt).cloned().unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        // Use real-time departure store
        let store = state.departure_store.read().await;
        store.get(&request.stop_ifopt).cloned().unwrap_or_default()
    };

    let reference = simulated_time.unwrap_or_else(Utc::now);
    let departures = filter_past_departures(departures, reference);

    Json(StopDeparturesResponse {
        stop_ifopt: request.stop_ifopt,
        departures,
    })
}
