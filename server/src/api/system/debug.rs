use crate::api::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VehicleIdAnalysis {
    pub total_stop_events: usize,
    pub unique_transportation_ids: usize,
    pub id_collisions: Vec<IdCollision>,
    pub sample_events: Vec<StopEventSample>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IdCollision {
    pub transportation_id: String,
    pub count: usize,
    pub examples: Vec<CollisionExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CollisionExample {
    pub station_id: String,
    pub line_number: String,
    pub destination: String,
    pub departure_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StopEventSample {
    pub station_id: String,
    pub transportation_id: String,
    pub trip_code: Option<i64>,
    pub vehicle_id: Option<String>,
    pub line_number: String,
    pub line_name: String,
    pub destination: String,
    pub origin: Option<String>,
    pub departure_planned: Option<String>,
    pub departure_estimated: Option<String>,
    pub additional_fields: serde_json::Value,
}

/// Debug endpoint to analyze vehicle ID uniqueness
///
/// This endpoint analyzes all stop events to identify potential ID collisions
/// and help determine the best strategy for unique vehicle identification.
#[utoipa::path(
    get,
    path = "/api/system/debug/vehicle-ids",
    responses(
        (status = 200, description = "Analysis of vehicle ID uniqueness", body = VehicleIdAnalysis)
    ),
    tag = "system"
)]
pub async fn analyze_vehicle_ids(State(state): State<AppState>) -> Response {
    // Acquire read lock on stop events cache
    let stop_events_cache = match state.stop_events.read() {
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

    let mut total_stop_events = 0;
    let mut id_map: HashMap<String, Vec<CollisionExample>> = HashMap::new();
    let mut samples = Vec::new();

    // Collect all transportation IDs and their contexts
    for (station_id, response) in stop_events_cache.iter() {
        for stop_event in &response.stop_events {
            total_stop_events += 1;

            let transportation_id = stop_event.transportation.id.clone();

            let example = CollisionExample {
                station_id: station_id.clone(),
                line_number: stop_event.transportation.number.clone(),
                destination: stop_event.transportation.destination.name.clone(),
                departure_time: stop_event
                    .departure_time_planned
                    .clone()
                    .or_else(|| stop_event.departure_time_estimated.clone()),
            };

            id_map
                .entry(transportation_id.clone())
                .or_insert_with(Vec::new)
                .push(example);

            // Collect first 20 samples to see variety
            if samples.len() < 20 {
                samples.push(StopEventSample {
                    station_id: station_id.clone(),
                    transportation_id: transportation_id.clone(),
                    trip_code: stop_event.transportation.trip_code,
                    vehicle_id: stop_event.transportation.vehicle_id.clone(),
                    line_number: stop_event.transportation.number.clone(),
                    line_name: stop_event.transportation.name.clone(),
                    destination: stop_event.transportation.destination.name.clone(),
                    origin: stop_event
                        .transportation
                        .origin
                        .as_ref()
                        .map(|o| o.name.clone()),
                    departure_planned: stop_event.departure_time_planned.clone(),
                    departure_estimated: stop_event.departure_time_estimated.clone(),
                    additional_fields: stop_event.transportation.additional_fields.clone(),
                });
            }
        }
    }

    let unique_transportation_ids = id_map.len();

    // Find collisions (same ID appearing in different contexts)
    let mut id_collisions = Vec::new();
    for (id, examples) in id_map.iter() {
        if examples.len() > 1 {
            // Check if these are actually different vehicles (different lines or destinations)
            let unique_contexts: std::collections::HashSet<String> = examples
                .iter()
                .map(|e| format!("{}:{}", e.line_number, e.destination))
                .collect();

            if unique_contexts.len() > 1 {
                id_collisions.push(IdCollision {
                    transportation_id: id.clone(),
                    count: examples.len(),
                    examples: examples.iter().take(5).cloned().collect(),
                });
            }
        }
    }

    // Sort collisions by count (most problematic first)
    id_collisions.sort_by(|a, b| b.count.cmp(&a.count));

    let analysis = VehicleIdAnalysis {
        total_stop_events,
        unique_transportation_ids,
        id_collisions,
        sample_events: samples,
    };

    Json(analysis).into_response()
}
