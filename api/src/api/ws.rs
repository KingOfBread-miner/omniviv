use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use chrono::{DateTime, Duration, Utc};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use tokio::sync::broadcast;

use super::vehicles::{Vehicle, VehicleStop};
use crate::providers::timetables::gtfs::realtime;
use crate::sync::{DepartureStore, EventType, ScheduleStore, VehicleUpdateSender};

#[derive(Clone)]
pub struct WsState {
    pub pool: SqlitePool,
    pub departure_store: DepartureStore,
    pub schedule_store: ScheduleStore,
    pub time_horizon_minutes: u32,
    pub timezone: chrono_tz::Tz,
    pub vehicle_updates_tx: VehicleUpdateSender,
}

/// Client subscription message
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ClientMessage {
    /// Subscribe to specific routes
    Subscribe {
        route_ids: Vec<i64>,
        /// Optional reference time for time simulation (ISO 8601/RFC 3339)
        reference_time: Option<String>,
    },
}

/// Server message sent to clients
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ServerMessage {
    /// Initial connection acknowledgment
    Connected { message: String },
    /// Full vehicle data (sent on initial subscribe)
    Vehicles { routes: Vec<RouteVehicles> },
    /// Incremental update with only changes
    VehiclesUpdate { changes: Vec<VehicleChange> },
    /// Error message
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
struct RouteVehicles {
    route_id: i64,
    line_number: Option<String>,
    vehicles: Vec<Vehicle>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
enum VehicleChange {
    /// A new vehicle appeared
    Add { route_id: i64, vehicle: Vehicle },
    /// A vehicle was updated (stops/times changed)
    Update { route_id: i64, vehicle: Vehicle },
    /// A vehicle was removed
    Remove { route_id: i64, trip_id: String },
}

/// Compute a hash for a single vehicle for change detection
fn compute_vehicle_hash(vehicle: &Vehicle) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    vehicle.trip_id.hash(&mut hasher);
    vehicle.line_number.hash(&mut hasher);
    vehicle.destination.hash(&mut hasher);
    for stop in &vehicle.stops {
        stop.stop_ifopt.hash(&mut hasher);
        stop.delay_minutes.hash(&mut hasher);
        stop.departure_time.hash(&mut hasher);
        stop.departure_time_estimated.hash(&mut hasher);
        stop.arrival_time.hash(&mut hasher);
        stop.arrival_time_estimated.hash(&mut hasher);
    }
    hasher.finish()
}

/// Previous state tracking for a connection
#[derive(Default)]
struct PreviousState {
    /// Map of (route_id, trip_id) -> vehicle hash
    vehicle_hashes: HashMap<(i64, String), u64>,
}

/// Compute changes between previous and current state
fn compute_changes(
    previous: &mut PreviousState,
    current: &[RouteVehicles],
) -> Vec<VehicleChange> {
    let mut changes = Vec::new();
    let mut seen_keys: HashSet<(i64, String)> = HashSet::new();

    // Check for new/updated vehicles
    for route in current {
        for vehicle in &route.vehicles {
            let key = (route.route_id, vehicle.trip_id.clone());
            seen_keys.insert(key.clone());

            let new_hash = compute_vehicle_hash(vehicle);

            match previous.vehicle_hashes.get(&key) {
                Some(&old_hash) if old_hash == new_hash => {
                    // No change
                }
                Some(_) => {
                    // Updated
                    changes.push(VehicleChange::Update {
                        route_id: route.route_id,
                        vehicle: vehicle.clone(),
                    });
                    previous.vehicle_hashes.insert(key, new_hash);
                }
                None => {
                    // New vehicle
                    changes.push(VehicleChange::Add {
                        route_id: route.route_id,
                        vehicle: vehicle.clone(),
                    });
                    previous.vehicle_hashes.insert(key, new_hash);
                }
            }
        }
    }

    // Check for removed vehicles
    let removed_keys: Vec<_> = previous
        .vehicle_hashes
        .keys()
        .filter(|k| !seen_keys.contains(*k))
        .cloned()
        .collect();

    for key in removed_keys {
        changes.push(VehicleChange::Remove {
            route_id: key.0,
            trip_id: key.1.clone(),
        });
        previous.vehicle_hashes.remove(&key);
    }

    changes
}

/// WebSocket endpoint for vehicle updates
pub async fn ws_vehicles(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: WsState) {
    let (mut sender, mut receiver) = socket.split();
    let mut vehicle_rx = state.vehicle_updates_tx.subscribe();
    let mut subscribed_routes: HashSet<i64> = HashSet::new();
    let mut previous_state = PreviousState::default();

    // Send connected message
    let connected_msg = ServerMessage::Connected {
        message: "Connected to vehicle updates. Send subscribe message with route_ids.".to_string(),
    };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Channel to communicate subscriptions from receiver task to sender task
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel::<(Vec<i64>, Option<String>)>(16);

    // Clone state for the forward task
    let forward_state = state.clone();

    let mut simulated_time: Option<DateTime<Utc>> = None;

    // Spawn task to forward broadcast updates to WebSocket
    let forward_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle subscription updates
                Some((route_ids, ref_time)) = sub_rx.recv() => {
                    subscribed_routes = route_ids.iter().copied().collect();
                    simulated_time = parse_ws_reference_time(&ref_time);
                    // Reset previous state when subscription changes
                    previous_state = PreviousState::default();

                    // Send full data for newly subscribed routes
                    if !subscribed_routes.is_empty() {
                        let routes: Vec<i64> = subscribed_routes.iter().copied().collect();
                        match build_vehicle_data(&forward_state, &routes, simulated_time).await {
                            Ok(data) => {
                                // Initialize previous state with current data
                                for route in &data {
                                    for vehicle in &route.vehicles {
                                        let key = (route.route_id, vehicle.trip_id.clone());
                                        let hash = compute_vehicle_hash(vehicle);
                                        previous_state.vehicle_hashes.insert(key, hash);
                                    }
                                }
                                let msg = ServerMessage::Vehicles { routes: data };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    if sender.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                let msg = ServerMessage::Error { message: e };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = sender.send(Message::Text(json.into())).await;
                                }
                            }
                        }
                    }
                }
                // Handle broadcast updates
                result = vehicle_rx.recv() => {
                    match result {
                        Ok(_update) => {
                            if subscribed_routes.is_empty() {
                                continue;
                            }
                            // For simulated time, skip broadcast updates (schedule data doesn't change)
                            if simulated_time.is_some() {
                                continue;
                            }
                            let routes: Vec<i64> = subscribed_routes.iter().copied().collect();
                            match build_vehicle_data(&forward_state, &routes, None).await {
                                Ok(data) => {
                                    // Compute changes from previous state
                                    let changes = compute_changes(&mut previous_state, &data);

                                    // Only send if there are actual changes
                                    if !changes.is_empty() {
                                        let msg = ServerMessage::VehiclesUpdate { changes };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            if sender.send(Message::Text(json.into())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to build vehicle data: {}", e);
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
            }
        }
    });

    // Handle incoming messages from client
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Subscribe { route_ids, reference_time } => {
                            let _ = sub_tx.send((route_ids, reference_time)).await;
                        }
                    }
                }
            }
            Ok(Message::Ping(_)) => {
                // Axum handles pong automatically
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Cleanup
    forward_task.abort();
}

#[derive(Debug, sqlx::FromRow)]
struct RouteInfo {
    line_ref: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct RouteStopInfo {
    sequence: i64,
    stop_ifopt: Option<String>,
    stop_name: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

/// Build vehicle data for the given routes
async fn build_vehicle_data(
    state: &WsState,
    route_ids: &[i64],
    simulated_time: Option<DateTime<Utc>>,
) -> Result<Vec<RouteVehicles>, String> {
    let mut results = Vec::new();

    for &route_id in route_ids {
        // Get route info
        let route_info: Option<RouteInfo> = sqlx::query_as(
            "SELECT ref as line_ref FROM routes WHERE osm_id = ?",
        )
        .bind(route_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let route_info = match route_info {
            Some(r) => r,
            None => continue, // Skip unknown routes
        };

        // Get route stops
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
        .bind(route_id)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Build stop info map
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
            results.push(RouteVehicles {
                route_id,
                line_number: route_info.line_ref,
                vehicles: vec![],
            });
            continue;
        }

        // Get departures either from store (real-time) or schedule (simulated time)
        let trip_departures: HashMap<String, Vec<crate::sync::Departure>> = if let Some(ref_time) = simulated_time {
            let schedule_guard = state.schedule_store.read().await;
            match schedule_guard.as_ref() {
                Some(schedule) => {
                    let stop_ids: HashSet<String> = stop_ifopts.iter().map(|s| s.to_string()).collect();
                    let time_horizon = Duration::minutes(state.time_horizon_minutes as i64);
                    let all_departures = realtime::compute_schedule_departures(
                        schedule,
                        &stop_ids,
                        ref_time,
                        time_horizon,
                        state.timezone,
                    );
                    let mut result: HashMap<String, Vec<crate::sync::Departure>> = HashMap::new();
                    for ifopt in &stop_ifopts {
                        if let Some(departures) = all_departures.get(*ifopt) {
                            for dep in departures {
                                let trip_id = match &dep.trip_id {
                                    Some(id) => id,
                                    None => continue,
                                };
                                if let Some(ref line_ref) = route_info.line_ref {
                                    if &dep.line_number != line_ref {
                                        continue;
                                    }
                                }
                                result.entry(trip_id.clone()).or_default().push(dep.clone());
                            }
                        }
                    }
                    result
                }
                None => HashMap::new(),
            }
        } else {
            let store = state.departure_store.read().await;
            let mut result: HashMap<String, Vec<crate::sync::Departure>> = HashMap::new();

            for ifopt in &stop_ifopts {
                if let Some(departures) = store.get(*ifopt) {
                    for dep in departures {
                        let trip_id = match &dep.trip_id {
                            Some(id) => id,
                            None => continue,
                        };

                        if let Some(ref line_ref) = route_info.line_ref {
                            if &dep.line_number != line_ref {
                                continue;
                            }
                        }

                        result.entry(trip_id.clone()).or_default().push(dep.clone());
                    }
                }
            }
            result
        };

        // Build vehicles
        let mut vehicles: Vec<Vehicle> = trip_departures
            .into_iter()
            .filter_map(|(trip_id, departures)| {
                if departures.is_empty() {
                    return None;
                }

                let line_number = departures.first()?.line_number.clone();

                let destination = departures
                    .iter()
                    .find(|d| d.event_type == EventType::Departure)
                    .map(|d| d.destination.clone())
                    .or_else(|| departures.first().map(|d| d.destination.clone()))?;

                let origin = departures
                    .iter()
                    .find(|d| d.event_type == EventType::Arrival)
                    .map(|d| d.destination.clone());

                // Group by stop
                let mut stop_events: HashMap<String, (Option<crate::sync::Departure>, Option<crate::sync::Departure>)> =
                    HashMap::new();

                for dep in departures {
                    let entry = stop_events.entry(dep.stop_ifopt.clone()).or_default();
                    match dep.event_type {
                        EventType::Arrival => entry.0 = Some(dep),
                        EventType::Departure => entry.1 = Some(dep),
                    }
                }

                let mut stops: Vec<VehicleStop> = stop_events
                    .into_iter()
                    .filter_map(|(stop_ifopt, (arrival, departure))| {
                        let (sequence, stop_name, lat, lon) = stop_info_map.get(&stop_ifopt)?;

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

        vehicles.sort_by(|a, b| {
            let time_a = a.stops.first().and_then(|s| s.departure_time.as_ref());
            let time_b = b.stops.first().and_then(|s| s.departure_time.as_ref());
            time_a.cmp(&time_b)
        });

        results.push(RouteVehicles {
            route_id,
            line_number: route_info.line_ref,
            vehicles,
        });
    }

    Ok(results)
}

/// Parse a reference_time string from a WebSocket message.
fn parse_ws_reference_time(reference_time: &Option<String>) -> Option<DateTime<Utc>> {
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

