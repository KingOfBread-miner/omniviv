use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use utoipa::ToSchema;

use super::VehiclesState;
use crate::api::ErrorResponse;
use crate::sync::{Departure, EventType};

#[derive(Debug, Deserialize, ToSchema)]
pub struct VehiclesByRouteRequest {
    /// The OSM route ID to get vehicles for
    pub route_id: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VehiclesByRouteResponse {
    pub route_id: i64,
    pub line_number: Option<String>,
    pub vehicles: Vec<Vehicle>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Vehicle {
    /// Unique trip identifier (AVMSTripID from EFA)
    pub trip_id: String,
    /// Line number (e.g., "1", "2", "3")
    pub line_number: String,
    /// Final destination of this vehicle
    pub destination: String,
    /// Origin of this vehicle's journey
    pub origin: Option<String>,
    /// All stops this vehicle will visit, in order
    pub stops: Vec<VehicleStop>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VehicleStop {
    /// Stop IFOPT identifier
    pub stop_ifopt: String,
    /// Stop name (if available)
    pub stop_name: Option<String>,
    /// Sequence number on the route
    pub sequence: i64,
    /// Latitude
    pub lat: f64,
    /// Longitude
    pub lon: f64,
    /// Arrival time at this stop (ISO 8601)
    pub arrival_time: Option<String>,
    /// Estimated arrival time (real-time, if available)
    pub arrival_time_estimated: Option<String>,
    /// Departure time from this stop (ISO 8601)
    pub departure_time: Option<String>,
    /// Estimated departure time (real-time, if available)
    pub departure_time_estimated: Option<String>,
    /// Delay in minutes (positive = late, negative = early)
    pub delay_minutes: Option<i32>,
}

#[derive(Debug, FromRow)]
struct RouteStopInfo {
    sequence: i64,
    stop_ifopt: Option<String>,
    stop_name: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

#[derive(Debug, FromRow)]
struct RouteInfo {
    line_ref: Option<String>,
}

/// Get all vehicles currently on a route with their stop sequences
#[utoipa::path(
    post,
    path = "/api/vehicles/by-route",
    request_body = VehiclesByRouteRequest,
    responses(
        (status = 200, description = "List of vehicles on the route", body = VehiclesByRouteResponse),
        (status = 404, description = "Route not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "vehicles"
)]
pub async fn get_vehicles_by_route(
    State(state): State<VehiclesState>,
    Json(request): Json<VehiclesByRouteRequest>,
) -> Result<Json<VehiclesByRouteResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get route info
    let route_info: Option<RouteInfo> = sqlx::query_as(
        "SELECT ref as line_ref FROM routes WHERE osm_id = ?",
    )
    .bind(request.route_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    let route_info = route_info.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Route not found".to_string(),
            }),
        )
    })?;

    // Get all stops on this route with their IFOPTs and coordinates
    let route_stops: Vec<RouteStopInfo> = sqlx::query_as(
        r#"
        SELECT
            rs.sequence,
            COALESCE(sp.ref_ifopt, p.ref_ifopt, st.ref_ifopt) as stop_ifopt,
            COALESCE(sp.name, p.name, st.name) as stop_name,
            COALESCE(sp.lat, p.lat, st.lat) as lat,
            COALESCE(sp.lon, p.lon, st.lon) as lon
        FROM route_stops rs
        LEFT JOIN stop_positions sp ON rs.stop_position_id = sp.osm_id
        LEFT JOIN platforms p ON rs.platform_id = p.osm_id
        LEFT JOIN stations st ON rs.station_id = st.osm_id
        WHERE rs.route_id = ?
        ORDER BY rs.sequence
        "#,
    )
    .bind(request.route_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Database error: {}", e),
            }),
        )
    })?;

    // Build a map of stop_ifopt -> (sequence, name, lat, lon)
    let stop_info_map: HashMap<String, (i64, Option<String>, f64, f64)> = route_stops
        .iter()
        .filter_map(|s| {
            let ifopt = s.stop_ifopt.as_ref()?;
            let lat = s.lat?;
            let lon = s.lon?;
            Some((ifopt.clone(), (s.sequence, s.stop_name.clone(), lat, lon)))
        })
        .collect();

    let stop_ifopts: Vec<&str> = stop_info_map.keys().map(|s| s.as_str()).collect();

    if stop_ifopts.is_empty() {
        return Ok(Json(VehiclesByRouteResponse {
            route_id: request.route_id,
            line_number: route_info.line_ref,
            vehicles: vec![],
        }));
    }

    // Get departures from the store and filter by route stops
    // Clone the data to release the lock quickly
    let trip_departures: HashMap<String, Vec<Departure>> = {
        let store = state.departure_store.read().await;

        let mut result: HashMap<String, Vec<Departure>> = HashMap::new();

        for ifopt in &stop_ifopts {
            if let Some(departures) = store.get(*ifopt) {
                for dep in departures {
                    // Skip if no trip_id
                    let trip_id = match &dep.trip_id {
                        Some(id) => id,
                        None => continue,
                    };

                    // If we know the line number, filter by it
                    if let Some(ref line_ref) = route_info.line_ref {
                        if &dep.line_number != line_ref {
                            continue;
                        }
                    }

                    result
                        .entry(trip_id.clone())
                        .or_default()
                        .push(dep.clone());
                }
            }
        }

        result
    };

    // Build vehicles from grouped departures
    let mut vehicles: Vec<Vehicle> = trip_departures
        .into_iter()
        .filter_map(|(trip_id, departures)| {
            if departures.is_empty() {
                return None;
            }

            // Get line number from first departure
            let line_number = departures.first()?.line_number.clone();

            // Find destination (from departures) and origin (from arrivals)
            let destination = departures
                .iter()
                .find(|d| d.event_type == EventType::Departure)
                .map(|d| d.destination.clone())
                .or_else(|| departures.first().map(|d| d.destination.clone()))?;

            let origin = departures
                .iter()
                .find(|d| d.event_type == EventType::Arrival)
                .map(|d| d.destination.clone()); // For arrivals, destination field contains origin

            // Group by stop to combine arrivals and departures
            let mut stop_events: HashMap<String, (Option<Departure>, Option<Departure>)> =
                HashMap::new();

            for dep in departures {
                let entry = stop_events.entry(dep.stop_ifopt.clone()).or_default();
                match dep.event_type {
                    EventType::Arrival => entry.0 = Some(dep),
                    EventType::Departure => entry.1 = Some(dep),
                }
            }

            // Build vehicle stops
            let mut stops: Vec<VehicleStop> = stop_events
                .into_iter()
                .filter_map(|(stop_ifopt, (arrival, departure))| {
                    let (sequence, stop_name, lat, lon) = stop_info_map.get(&stop_ifopt)?;

                    // Get delay from whichever event is available
                    let delay_minutes = departure
                        .as_ref()
                        .and_then(|d| d.delay_minutes)
                        .or_else(|| arrival.as_ref().and_then(|a| a.delay_minutes));

                    Some(VehicleStop {
                        stop_ifopt,
                        stop_name: stop_name.clone(),
                        sequence: *sequence,
                        lat: *lat,
                        lon: *lon,
                        arrival_time: arrival.as_ref().map(|a| a.planned_time.clone()),
                        arrival_time_estimated: arrival.as_ref().and_then(|a| a.estimated_time.clone()),
                        departure_time: departure.as_ref().map(|d| d.planned_time.clone()),
                        departure_time_estimated: departure.as_ref().and_then(|d| d.estimated_time.clone()),
                        delay_minutes,
                    })
                })
                .collect();

            // Sort stops by sequence
            stops.sort_by_key(|s| s.sequence);

            Some(Vehicle {
                trip_id,
                line_number,
                destination,
                origin,
                stops,
            })
        })
        .collect();

    // Sort vehicles by their first stop's departure time
    vehicles.sort_by(|a, b| {
        let time_a = a.stops.first().and_then(|s| s.departure_time.as_ref());
        let time_b = b.stops.first().and_then(|s| s.departure_time.as_ref());
        time_a.cmp(&time_b)
    });

    Ok(Json(VehiclesByRouteResponse {
        route_id: request.route_id,
        line_number: route_info.line_ref,
        vehicles,
    }))
}
