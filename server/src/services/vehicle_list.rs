/// Vehicle listing service
///
/// This module extracts a list of all unique vehicles currently on the network
/// from the stop events cache. This is a simpler operation than full position
/// estimation - it just identifies which vehicles are active.
///
/// Vehicle IDs are composed of: transportationId_destinationId (e.g., "avg:03003: :H:j25_de:09761:200")
/// Only vehicles with a tripCode are tracked.
use crate::models::{VehicleInfo, VehicleListResponse};
use crate::services::efa::EfaDepartureMonitorResponse;
use std::collections::HashMap;
use tracing::info;

/// Extract all unique vehicles from stop events cache with stale tracking
///
/// This function scans all stop events from all stations and builds a HashMap
/// of unique vehicles identified by combining their transportation.id and destination.id.
/// Only vehicles with a tripCode are tracked. It maintains state from previous calls,
/// marking vehicles as stale if they're not found in the current query, and removing
/// them after a configurable timeout.
///
/// # Arguments
/// * `stop_events` - HashMap of station_id -> EfaDepartureMonitorResponse
/// * `previous_vehicles` - Optional previous vehicle list to maintain state
/// * `stale_timeout_seconds` - How long to keep stale vehicles before removing (default: 300s = 5min)
///
/// # Returns
/// VehicleListResponse containing all unique vehicles with stale tracking
pub fn extract_unique_vehicles(
    stop_events: &HashMap<String, EfaDepartureMonitorResponse>,
    previous_vehicles: Option<&HashMap<String, VehicleInfo>>,
    stale_timeout_seconds: u64,
) -> VehicleListResponse {
    let now = chrono::Utc::now();
    let timestamp = now.to_rfc3339();
    let mut vehicles: HashMap<String, VehicleInfo> = HashMap::new();

    // Build set of vehicle IDs found in current query
    let mut found_vehicle_ids = std::collections::HashSet::new();

    let total_stop_events: usize = stop_events.values().map(|r| r.stop_events.len()).sum();
    info!(
        "Extracting unique vehicles from {} stations with {} total stop events",
        stop_events.len(),
        total_stop_events
    );

    let mut skipped_duplicate = 0;
    let mut skipped_no_tripcode = 0;
    let mut new_vehicles = 0;
    let mut updated_vehicles = 0;

    // Process each station's stop events
    for (station_id, response) in stop_events {
        for stop_event in &response.stop_events {
            // Only track vehicles with tripCode
            if stop_event.transportation.trip_code.is_none() {
                skipped_no_tripcode += 1;
                continue;
            }

            // Generate vehicle ID from transportation.id and destination.id
            let vehicle_id = match &stop_event.transportation.destination.id {
                Some(dest_id) => {
                    // Combine transportation.id and destination.id for unique identification
                    format!("{}_{}", stop_event.transportation.id, dest_id)
                }
                None => {
                    // Fallback: use transportation.id only if destination has no id
                    stop_event.transportation.id.clone()
                }
            };

            // Check if we already processed this vehicle in current query
            // Keep the first one we find
            if vehicles.contains_key(&vehicle_id) {
                skipped_duplicate += 1;
                continue;
            }

            found_vehicle_ids.insert(vehicle_id.clone());

            // Check if this vehicle existed before
            let first_seen = if let Some(prev_vehicles) = previous_vehicles {
                if let Some(prev_vehicle) = prev_vehicles.get(&vehicle_id) {
                    updated_vehicles += 1;
                    prev_vehicle.first_seen.clone()
                } else {
                    new_vehicles += 1;
                    timestamp.clone()
                }
            } else {
                new_vehicles += 1;
                timestamp.clone()
            };

            // Extract basic vehicle information
            let vehicle_info = VehicleInfo {
                vehicle_id: vehicle_id.clone(),
                trip_code: stop_event.transportation.trip_code,
                line_number: stop_event.transportation.number.clone(),
                line_name: stop_event.transportation.name.clone(),
                destination: stop_event.transportation.destination.name.clone(),
                origin: stop_event
                    .transportation
                    .origin
                    .as_ref()
                    .map(|o| o.name.clone()),
                is_stale: false,
                last_seen: timestamp.clone(),
                first_seen,
            };

            vehicles.insert(vehicle_id, vehicle_info);
        }
    }

    // Mark vehicles from previous state as stale if not found, or remove if too old
    let mut removed_vehicles = 0;

    if let Some(prev_vehicles) = previous_vehicles {
        for (vehicle_id, prev_vehicle) in prev_vehicles {
            // Skip if we already have this vehicle (it was found in current query)
            if vehicles.contains_key(vehicle_id) {
                continue;
            }

            // Check if this vehicle has been stale for too long
            if let Ok(last_seen_time) =
                chrono::DateTime::parse_from_rfc3339(&prev_vehicle.last_seen)
            {
                let stale_duration = now.signed_duration_since(last_seen_time);

                if stale_duration.num_seconds() > stale_timeout_seconds as i64 {
                    // Vehicle has been stale too long, don't include it
                    removed_vehicles += 1;
                    info!(
                        vehicle_id = %vehicle_id,
                        line = %prev_vehicle.line_number,
                        destination = %prev_vehicle.destination,
                        stale_duration_seconds = stale_duration.num_seconds(),
                        "Removing stale vehicle (timeout)"
                    );
                    continue;
                }
            }

            // Keep the vehicle but mark as stale
            let mut stale_vehicle = prev_vehicle.clone();
            stale_vehicle.is_stale = true;
            vehicles.insert(vehicle_id.clone(), stale_vehicle);
        }
    }

    let total_count = vehicles.len();
    let active_count = vehicles.values().filter(|v| !v.is_stale).count();
    let stale_count = vehicles.values().filter(|v| v.is_stale).count();

    info!(
        total_vehicles = total_count,
        active = active_count,
        stale = stale_count,
        new = new_vehicles,
        updated = updated_vehicles,
        removed = removed_vehicles,
        duplicates_skipped = skipped_duplicate,
        skipped_no_tripcode = skipped_no_tripcode,
        "Vehicle list updated (tripCode only)"
    );

    VehicleListResponse {
        vehicles,
        total_count,
        active_count,
        stale_count,
        timestamp,
    }
}
