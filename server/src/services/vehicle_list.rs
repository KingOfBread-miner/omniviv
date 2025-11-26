/// Vehicle listing service
///
/// This module extracts a list of all unique vehicles currently on the network
/// from the stop events cache using a time-window based approach.
///
/// # Approach
/// Instead of maintaining state over time, this service:
/// 1. Filters stop events by departure time (recent past + near future)
/// 2. Extracts unique vehicles from this time window
/// 3. Deduplicates by keeping the latest departure
///
/// This ensures we only show vehicles that are currently active or about to depart.
use crate::models::{VehicleInfo, VehicleListResponse};
use crate::services::efa::EfaDepartureMonitorResponse;
use std::collections::HashMap;
use tracing::info;

/// Extract currently active vehicles from stop events cache using time-window filtering
///
/// This function uses a stateless, time-window based approach to identify vehicles
/// that are currently on the network. Only stop events within a specific time window
/// (recent past + near future) are considered.
///
/// # Time Window
/// - Past: -20 minutes (vehicles that recently departed)
/// - Future: +20 minutes (vehicles about to depart)
/// - Total window: 40 minutes around current time
///
/// # Vehicle Identification
/// - Uses tripCode as the unique identifier (one trip = one vehicle entry)
/// - Physical vehicle_id stored separately if available
/// - Only tracks vehicles with tripCode
///
/// # Deduplication
/// When the same tripCode appears at multiple stops:
/// - Keeps the entry with the LATEST departure time
/// - Rationale: Vehicle progresses forward, latest = most current position
///
/// # Arguments
/// * `stop_events` - HashMap of station_id -> EfaDepartureMonitorResponse
///
/// # Returns
/// VehicleListResponse containing currently active vehicles
pub fn extract_unique_vehicles(
    stop_events: &HashMap<String, EfaDepartureMonitorResponse>,
    _previous_vehicles: Option<&HashMap<String, VehicleInfo>>,
    _stale_timeout_seconds: u64,
) -> VehicleListResponse {
    let now = chrono::Utc::now();
    let timestamp = now.to_rfc3339();
    let mut vehicles: HashMap<String, VehicleInfo> = HashMap::new();

    // Define time window for "currently active" vehicles
    let time_window_past_minutes = 20; // Include vehicles that departed up to 20 min ago
    let time_window_future_minutes = 20; // Include vehicles departing up to 20 min from now

    let window_start = now - chrono::Duration::minutes(time_window_past_minutes);
    let window_end = now + chrono::Duration::minutes(time_window_future_minutes);

    let total_stop_events: usize = stop_events.values().map(|r| r.stop_events.len()).sum();
    info!(
        stations = stop_events.len(),
        stop_events = total_stop_events,
        time_window = format!("-{} to +{} minutes", time_window_past_minutes, time_window_future_minutes),
        "Extracting currently active vehicles using time-window filter"
    );

    let mut skipped_no_tripcode = 0;
    let mut skipped_outside_window = 0;
    let mut replaced_with_later = 0;

    // Process each station's stop events
    for (_station_id, response) in stop_events {
        for stop_event in &response.stop_events {
            // Only track vehicles with tripCode
            let trip_code = match stop_event.transportation.trip_code {
                Some(tc) => tc,
                None => {
                    skipped_no_tripcode += 1;
                    continue;
                }
            };

            // Parse departure time for time-window filtering
            let departure_time_str = stop_event
                .departure_time_estimated
                .as_ref()
                .or(stop_event.departure_time_planned.as_ref());

            let departure_time = match departure_time_str.and_then(|dt_str| {
                chrono::DateTime::parse_from_rfc3339(dt_str)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            }) {
                Some(dt) => dt,
                None => {
                    // No valid departure time, skip
                    skipped_outside_window += 1;
                    continue;
                }
            };

            // TIME WINDOW FILTER: Only include vehicles within the active time window
            if departure_time < window_start || departure_time > window_end {
                skipped_outside_window += 1;
                continue;
            }

            // Generate vehicle ID from tripCode (one trip = one vehicle entry)
            let vehicle_id = trip_code.to_string();

            // Check if we already have this vehicle in current batch
            if let Some(existing) = vehicles.get(&vehicle_id) {
                // Compare departure times - keep the one with LATER departure
                let existing_departure = chrono::DateTime::parse_from_rfc3339(&existing.last_departure_planned)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc));

                if let Some(existing_dt) = existing_departure {
                    if departure_time > existing_dt {
                        // Current stop event is later, replace existing
                        replaced_with_later += 1;
                    } else {
                        // Existing is later or same, keep it
                        continue;
                    }
                }
            }

            // Extract next stop information (first in onward_locations)
            let (next_stop_id, next_stop_name) = stop_event
                .onward_locations
                .as_ref()
                .and_then(|locs| locs.first())
                .map(|loc| (Some(loc.id.clone()), Some(loc.name.clone())))
                .unwrap_or((None, None));

            // Extract stops ahead (future stops)
            let stops_ahead = stop_event
                .onward_locations
                .as_ref()
                .map(|locs| locs.iter().map(|loc| loc.id.clone()).collect())
                .unwrap_or_else(Vec::new);

            // Extract stops behind (past stops)
            let stops_behind = stop_event
                .previous_locations
                .as_ref()
                .map(|locs| locs.iter().map(|loc| loc.id.clone()).collect())
                .unwrap_or_else(Vec::new);

            // Build comprehensive vehicle information
            let vehicle_info = VehicleInfo {
                vehicle_id: vehicle_id.clone(),
                trip_code,
                physical_vehicle_id: stop_event.transportation.vehicle_id.clone(),

                line_number: stop_event.transportation.number.clone(),
                line_name: stop_event.transportation.name.clone(),
                destination: stop_event.transportation.destination.name.clone(),
                origin: stop_event
                    .transportation
                    .origin
                    .as_ref()
                    .map(|o| o.name.clone()),

                current_stop_id: stop_event.location.id.clone(),
                current_stop_name: stop_event.location.name.clone(),
                next_stop_id,
                next_stop_name,

                last_departure_planned: stop_event
                    .departure_time_planned
                    .clone()
                    .unwrap_or_else(|| timestamp.clone()),
                last_departure_estimated: stop_event.departure_time_estimated.clone(),
                delay_minutes: stop_event.departure_delay,

                is_stale: false, // Not used in time-window approach
                last_seen: timestamp.clone(),
                first_seen: timestamp.clone(), // Not tracked in stateless approach

                stops_ahead,
                stops_behind,
            };

            vehicles.insert(vehicle_id, vehicle_info);
        }
    }

    let total_count = vehicles.len();
    let active_count = total_count; // All vehicles in time window are active
    let stale_count = 0; // No stale tracking in time-window approach

    info!(
        total = total_count,
        active = active_count,
        replaced = replaced_with_later,
        skipped_no_tripcode = skipped_no_tripcode,
        skipped_outside_window = skipped_outside_window,
        "Vehicle list extracted using time-window filter"
    );

    VehicleListResponse {
        vehicles,
        total_count,
        active_count,
        stale_count,
        timestamp,
    }
}
