//! Background synchronization of OSM and EFA data.
//!
//! This module handles:
//! - Periodic synchronization of OSM transit data (stations, platforms, routes)
//! - Real-time departure/arrival data from EFA API
//! - OSM data quality issue detection and enrichment

mod issues;
mod types;

// Re-export types for API compatibility
pub use issues::{determine_transport_type, transport_type_from_route, OsmIssue, OsmIssueStore, OsmIssueType};
pub use types::{
    Departure, DepartureStore, EfaRequestLog, EfaRequestSender, EventType, VehicleUpdate,
    VehicleUpdateSender,
};

use crate::config::{Area, Config, TransportType};
use crate::providers::osm::{OsmClient, OsmElement, OsmRoute};
use crate::providers::timetables::germany::bavaria::EfaClient;
use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

/// Manages background synchronization of OSM and EFA data
pub struct SyncManager {
    pool: SqlitePool,
    osm_client: OsmClient,
    efa_client: EfaClient,
    config: Arc<RwLock<Config>>,
    departures: DepartureStore,
    issues: OsmIssueStore,
    vehicle_updates_tx: VehicleUpdateSender,
    efa_requests_tx: EfaRequestSender,
}

impl SyncManager {
    pub fn new(pool: SqlitePool, config: Config) -> Result<Self, SyncError> {
        let osm_client = OsmClient::new().map_err(|e| SyncError::OsmError(e.to_string()))?;

        // Create broadcast channel for EFA request diagnostics (capacity 100)
        let (efa_requests_tx, _) = broadcast::channel(100);

        let efa_client = EfaClient::with_max_concurrent(
            efa_requests_tx.clone(),
            config.efa_sync.max_concurrent_requests,
        )
        .map_err(|e| SyncError::EfaError(e.to_string()))?;

        // Create broadcast channel for vehicle updates (capacity 16 - clients will get latest state anyway)
        let (vehicle_updates_tx, _) = broadcast::channel(16);

        Ok(Self {
            pool,
            osm_client,
            efa_client,
            config: Arc::new(RwLock::new(config)),
            departures: Arc::new(RwLock::new(HashMap::new())),
            issues: Arc::new(RwLock::new(Vec::new())),
            vehicle_updates_tx,
            efa_requests_tx,
        })
    }

    /// Get a reference to the departure store for API access
    pub fn departure_store(&self) -> DepartureStore {
        self.departures.clone()
    }

    /// Get a reference to the OSM issue store for API access
    pub fn issue_store(&self) -> OsmIssueStore {
        self.issues.clone()
    }

    /// Get the vehicle updates sender for passing to API handlers
    pub fn vehicle_updates_sender(&self) -> VehicleUpdateSender {
        self.vehicle_updates_tx.clone()
    }

    /// Get the EFA request sender for passing to diagnostics WebSocket
    pub fn efa_requests_sender(&self) -> EfaRequestSender {
        self.efa_requests_tx.clone()
    }

    /// Start the background sync loops
    pub async fn start(self: Arc<Self>) {
        info!("Starting sync manager");

        // Initial OSM sync on startup
        self.sync_all_areas().await;

        // Spawn OSM sync loop (every 6 hours)
        let osm_self = self.clone();
        let osm_handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(6 * 60 * 60));
            // Skip the first tick which fires immediately (we already synced above)
            interval.tick().await;

            loop {
                interval.tick().await;
                osm_self.sync_all_areas().await;
            }
        });

        // Spawn departure sync loop (configurable interval)
        let efa_self = self.clone();
        let efa_handle = tokio::spawn(async move {
            // Wait a bit for initial OSM sync to complete
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            let interval_secs = {
                let config = efa_self.config.read().await;
                config.efa_sync.interval_secs
            };
            info!(interval_secs, "Starting EFA departure sync loop");
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;
                efa_self.sync_all_departures().await;
            }
        });

        // Wait for both loops (they run forever)
        let _ = tokio::join!(osm_handle, efa_handle);
    }

    /// Sync all areas from config
    async fn sync_all_areas(&self) {
        // Clear previous issues before starting new sync
        {
            let mut issues = self.issues.write().await;
            issues.clear();
        }

        let config = self.config.read().await;
        let areas = config.areas.clone();
        drop(config);

        for area in areas {
            let max_retries = 5;
            let mut attempt = 0;

            loop {
                attempt += 1;
                match self.sync_area(&area).await {
                    Ok(()) => break,
                    Err(e) => {
                        if attempt >= max_retries {
                            error!(area = %area.name, error = %e, attempts = attempt, "Failed to sync area after max retries, skipping");
                            break;
                        }
                        let wait_secs = 30 * attempt;
                        error!(area = %area.name, error = %e, attempt, wait_secs, "Failed to sync area, retrying...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs as u64)).await;
                    }
                }
            }
        }

        // After all areas are synced, enrich missing IFOPT issues with EFA suggestions
        self.enrich_issues_with_efa_suggestions().await;
    }

    /// Enrich missing IFOPT issues with suggested IFOPTs from EFA API
    async fn enrich_issues_with_efa_suggestions(&self) {
        let mut issues = self.issues.write().await;

        // Find all missing IFOPT issues with coordinates
        let missing_ifopt_indices: Vec<usize> = issues
            .iter()
            .enumerate()
            .filter(|(_, issue)| {
                matches!(issue.issue_type, OsmIssueType::MissingIfopt)
                    && issue.lat.is_some()
                    && issue.lon.is_some()
            })
            .map(|(i, _)| i)
            .collect();

        if missing_ifopt_indices.is_empty() {
            return;
        }

        info!(
            count = missing_ifopt_indices.len(),
            "Enriching missing IFOPT issues with EFA suggestions"
        );

        // Process each issue and query EFA
        for idx in missing_ifopt_indices {
            let issue = &issues[idx];
            let lat = issue.lat.unwrap();
            let lon = issue.lon.unwrap();
            let osm_name = issue.name.clone();
            let element_type = issue.element_type.clone();

            // Query EFA for nearby stops (200m radius)
            match self.efa_client.find_stops_by_coord(lon, lat, 200).await {
                Ok(response) => {
                    // For platforms and stop_positions, try to get platform-level IFOPT
                    if element_type == "platform" || element_type == "stop_position" {
                        if let Some(suggestion) = self.find_platform_ifopt_match(&response.locations, &osm_name).await {
                            issues[idx].suggested_ifopt = Some(suggestion.0);
                            issues[idx].suggested_ifopt_name = suggestion.1;
                            issues[idx].suggested_ifopt_distance = suggestion.2;
                            continue;
                        }
                    }

                    // Fallback to station-level IFOPT
                    if let Some(suggestion) = self.find_station_ifopt_match(&response.locations, &osm_name) {
                        issues[idx].suggested_ifopt = Some(suggestion.0);
                        issues[idx].suggested_ifopt_name = suggestion.1;
                        issues[idx].suggested_ifopt_distance = suggestion.2;
                    }
                }
                Err(e) => {
                    warn!(
                        osm_id = issue.osm_id,
                        error = %e,
                        "Failed to query EFA for IFOPT suggestion"
                    );
                }
            }

            // Small delay to avoid overwhelming the EFA API
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let enriched_count = issues
            .iter()
            .filter(|i| i.suggested_ifopt.is_some())
            .count();
        info!(
            enriched = enriched_count,
            total = issues.len(),
            "Finished enriching issues with EFA suggestions"
        );
    }

    /// Find platform-level IFOPT by querying departures for nearby stations
    /// Returns (ifopt, name, distance) if a good match is found
    async fn find_platform_ifopt_match(
        &self,
        stations: &[crate::providers::timetables::germany::bavaria::CoordLocation],
        osm_name: &Option<String>,
    ) -> Option<(String, Option<String>, Option<u32>)> {
        if stations.is_empty() {
            return None;
        }

        // Extract platform ref from OSM name if present (e.g., "a", "b", "1", "2")
        let osm_ref = osm_name.as_ref().and_then(|name| {
            // Look for patterns like "Bstg. a", "Platform a", or just single letter/number at end
            let name_lower = name.to_lowercase();
            if let Some(idx) = name_lower.rfind(|c: char| c.is_whitespace()) {
                let suffix = &name[idx + 1..];
                if suffix.len() <= 2 {
                    return Some(suffix.to_lowercase());
                }
            }
            // Check if name is just a single letter/number
            if name.len() <= 2 && name.chars().all(|c| c.is_alphanumeric()) {
                return Some(name.to_lowercase());
            }
            None
        });

        // Try the closest stations
        for station in stations.iter().take(3) {
            let station_ifopt = match station.ifopt() {
                Some(id) => id,
                None => continue,
            };
            let station_distance = station.distance_meters();

            // Query platforms for this station
            match self.efa_client.get_station_platforms(station_ifopt).await {
                Ok(platforms) => {
                    // If we have an OSM ref, try to match it to a platform
                    if let Some(ref osm_ref) = osm_ref {
                        for platform in &platforms {
                            if let Some(ref efa_ref) = platform.platform {
                                if efa_ref.to_lowercase() == *osm_ref {
                                    let name = platform.name.clone()
                                        .or_else(|| platform.station_name.clone());
                                    return Some((platform.ifopt.clone(), name, station_distance));
                                }
                            }
                        }
                    }

                    // If only one platform, suggest it
                    if platforms.len() == 1 {
                        let platform = &platforms[0];
                        let name = platform.name.clone()
                            .or_else(|| platform.station_name.clone());
                        return Some((platform.ifopt.clone(), name, station_distance));
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        station = station_ifopt,
                        error = %e,
                        "Failed to query platforms for station"
                    );
                }
            }
        }

        None
    }

    /// Find station-level IFOPT match from EFA locations
    /// Returns (ifopt, name, distance) if a good match is found
    fn find_station_ifopt_match(
        &self,
        locations: &[crate::providers::timetables::germany::bavaria::CoordLocation],
        osm_name: &Option<String>,
    ) -> Option<(String, Option<String>, Option<u32>)> {
        if locations.is_empty() {
            return None;
        }

        // If we have an OSM name, try to find a matching EFA stop
        if let Some(osm_name) = osm_name {
            let osm_name_lower = osm_name.to_lowercase();

            for loc in locations {
                if let Some(ifopt) = loc.ifopt() {
                    let distance = loc.distance_meters();

                    // Check if name matches (exact or partial)
                    let efa_name = loc.name.as_deref().unwrap_or("");
                    let efa_name_lower = efa_name.to_lowercase();

                    // Consider it a match if:
                    // 1. Names are exactly equal (case insensitive)
                    // 2. One name contains the other
                    // 3. Distance is very close (<50m) - likely the same stop
                    let name_matches = osm_name_lower == efa_name_lower
                        || osm_name_lower.contains(&efa_name_lower)
                        || efa_name_lower.contains(&osm_name_lower);

                    let very_close = distance.map_or(false, |d| d < 50);

                    if name_matches || very_close {
                        return Some((
                            ifopt.to_string(),
                            loc.full_name().map(|s| s.to_string()),
                            distance,
                        ));
                    }
                }
            }
        }

        // If no name match, use the closest stop if within 100m
        let closest = locations.first()?;
        let distance = closest.distance_meters()?;

        if distance <= 100 {
            if let Some(ifopt) = closest.ifopt() {
                return Some((
                    ifopt.to_string(),
                    closest.full_name().map(|s| s.to_string()),
                    Some(distance),
                ));
            }
        }

        None
    }

    /// Sync a single area (all database operations in a single transaction)
    async fn sync_area(&self, area: &Area) -> Result<(), SyncError> {
        info!(area = %area.name, "Starting sync for area");

        // Fetch features from OSM first (before starting transaction)
        let features = self
            .osm_client
            .fetch_area_features(area)
            .await
            .map_err(|e| SyncError::OsmError(e.to_string()))?;

        // Extract platform->station mappings from stop_area relations
        let platform_station_map = OsmClient::extract_station_platform_mappings(&features.stations);

        info!(
            area = %area.name,
            stations = features.stations.len(),
            platforms = features.platforms.len(),
            stop_positions = features.stop_positions.len(),
            routes = features.routes.len(),
            platform_mappings = platform_station_map.len(),
            "Fetched features from OSM"
        );

        // Start a single transaction for all database operations
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Ensure area exists in database
        let area_id = self.upsert_area(&mut tx, area).await?;

        // Store features in database
        self.store_stations(&mut tx, &features.stations, area_id).await?;
        self.store_platforms(&mut tx, &features.platforms, area_id, &platform_station_map).await?;
        self.store_stop_positions(&mut tx, &features.stop_positions, area_id, &platform_station_map).await?;
        self.store_routes(&mut tx, &features.routes, area_id).await?;

        // Resolve remaining relations (fallback for unmapped platforms)
        self.resolve_relations(&mut tx, area_id).await?;

        // Check for missing platform/stop_position pairs
        self.check_platform_stop_pairs(&mut tx, area_id).await?;

        // Update last_synced_at
        sqlx::query("UPDATE areas SET last_synced_at = datetime('now') WHERE id = ?")
            .bind(area_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Commit all changes atomically
        tx.commit()
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        info!(area = %area.name, "Completed sync for area");
        Ok(())
    }

    /// Insert or update area in database
    async fn upsert_area(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        area: &Area,
    ) -> Result<i64, SyncError> {
        let result = sqlx::query(
            r#"
            INSERT INTO areas (name, south, west, north, east)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                south = excluded.south,
                west = excluded.west,
                north = excluded.north,
                east = excluded.east
            RETURNING id
            "#,
        )
        .bind(&area.name)
        .bind(area.bounding_box.south)
        .bind(area.bounding_box.west)
        .bind(area.bounding_box.north)
        .bind(area.bounding_box.east)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        Ok(sqlx::Row::get(&result, "id"))
    }

    /// Store stations in database
    async fn store_stations(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        stations: &[OsmElement],
        area_id: i64,
    ) -> Result<(), SyncError> {
        let mut new_issues = Vec::new();

        for station in stations {
            let name = station.tag("name").map(|s| s.to_string());
            let lat = station.latitude();
            let lon = station.longitude();
            let transport_type = determine_transport_type(station);

            // Check for missing coordinates
            let (lat, lon) = match (lat, lon) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => {
                    new_issues.push(OsmIssue::new(
                        station.id,
                        &station.element_type,
                        "station",
                        OsmIssueType::MissingCoordinates,
                        transport_type.clone(),
                        format!("Station '{}' has no coordinates", name.as_deref().unwrap_or("unnamed")),
                        name,
                        None, // ref_tag
                        None,
                        None,
                    ));
                    continue;
                }
            };

            // Check for missing IFOPT
            if station.tag("ref:IFOPT").is_none() {
                new_issues.push(OsmIssue::new(
                    station.id,
                    &station.element_type,
                    "station",
                    OsmIssueType::MissingIfopt,
                    transport_type,
                    format!("Station '{}' has no ref:IFOPT tag", name.as_deref().unwrap_or("unnamed")),
                    name.clone(),
                    None, // ref_tag
                    Some(lat),
                    Some(lon),
                ));
            }

            let tags_json = station.tags.as_ref().and_then(|t| {
                serde_json::to_string(t)
                    .map_err(|e| tracing::warn!(osm_id = station.id, error = %e, "Failed to serialize station tags"))
                    .ok()
            });

            sqlx::query(
                r#"
                INSERT INTO stations (osm_id, osm_type, name, ref_ifopt, lat, lon, tags, area_id, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(osm_id) DO UPDATE SET
                    osm_type = excluded.osm_type,
                    name = excluded.name,
                    ref_ifopt = excluded.ref_ifopt,
                    lat = excluded.lat,
                    lon = excluded.lon,
                    tags = excluded.tags,
                    area_id = excluded.area_id,
                    updated_at = datetime('now')
                "#,
            )
            .bind(station.id)
            .bind(&station.element_type)
            .bind(station.tag("name"))
            .bind(station.tag("ref:IFOPT"))
            .bind(lat)
            .bind(lon)
            .bind(tags_json)
            .bind(area_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
        }

        // Store collected issues
        if !new_issues.is_empty() {
            let mut issues = self.issues.write().await;
            issues.extend(new_issues);
        }

        Ok(())
    }

    /// Store platforms in database with optional station mapping from stop_area relations
    async fn store_platforms(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        platforms: &[OsmElement],
        area_id: i64,
        platform_station_map: &HashMap<i64, i64>,
    ) -> Result<(), SyncError> {
        let mut new_issues = Vec::new();

        for platform in platforms {
            let name = platform.tag("name").map(|s| s.to_string());
            let platform_ref = platform.tag("ref").map(|s| s.to_string());
            let lat = platform.latitude();
            let lon = platform.longitude();
            let transport_type = determine_transport_type(platform);

            // Check for missing coordinates
            let (lat, lon) = match (lat, lon) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => {
                    new_issues.push(OsmIssue::new(
                        platform.id,
                        &platform.element_type,
                        "platform",
                        OsmIssueType::MissingCoordinates,
                        transport_type.clone(),
                        format!("Platform '{}' has no coordinates", name.as_deref().unwrap_or("unnamed")),
                        name,
                        platform_ref,
                        None,
                        None,
                    ));
                    continue;
                }
            };

            // Check for missing IFOPT
            if platform.tag("ref:IFOPT").is_none() {
                new_issues.push(OsmIssue::new(
                    platform.id,
                    &platform.element_type,
                    "platform",
                    OsmIssueType::MissingIfopt,
                    transport_type.clone(),
                    format!("Platform '{}' has no ref:IFOPT tag", name.as_deref().unwrap_or("unnamed")),
                    name.clone(),
                    platform_ref.clone(),
                    Some(lat),
                    Some(lon),
                ));
            }

            // Check for missing name and ref (would show as "?" on map)
            if name.is_none() && platform_ref.is_none() {
                new_issues.push(OsmIssue::new(
                    platform.id,
                    &platform.element_type,
                    "platform",
                    OsmIssueType::MissingName,
                    transport_type,
                    "Platform has no name or ref tag - displays as '?' on map".to_string(),
                    None,
                    None,
                    Some(lat),
                    Some(lon),
                ));
            }

            let tags_json = platform.tags.as_ref().and_then(|t| {
                serde_json::to_string(t)
                    .map_err(|e| tracing::warn!(osm_id = platform.id, error = %e, "Failed to serialize platform tags"))
                    .ok()
            });

            // Get station_id from stop_area membership
            let station_id = platform_station_map.get(&platform.id).copied();

            sqlx::query(
                r#"
                INSERT INTO platforms (osm_id, osm_type, name, ref, ref_ifopt, lat, lon, tags, station_id, area_id, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(osm_id) DO UPDATE SET
                    osm_type = excluded.osm_type,
                    name = excluded.name,
                    ref = excluded.ref,
                    ref_ifopt = excluded.ref_ifopt,
                    lat = excluded.lat,
                    lon = excluded.lon,
                    tags = excluded.tags,
                    station_id = COALESCE(excluded.station_id, platforms.station_id),
                    area_id = excluded.area_id,
                    updated_at = datetime('now')
                "#,
            )
            .bind(platform.id)
            .bind(&platform.element_type)
            .bind(platform.tag("name"))
            .bind(platform.tag("ref"))
            .bind(platform.tag("ref:IFOPT"))
            .bind(lat)
            .bind(lon)
            .bind(tags_json)
            .bind(station_id)
            .bind(area_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
        }

        // Store collected issues
        if !new_issues.is_empty() {
            let mut issues = self.issues.write().await;
            issues.extend(new_issues);
        }

        Ok(())
    }

    /// Store stop positions in database with optional station mapping from stop_area relations
    async fn store_stop_positions(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        stop_positions: &[OsmElement],
        area_id: i64,
        platform_station_map: &HashMap<i64, i64>,
    ) -> Result<(), SyncError> {
        let mut new_issues = Vec::new();

        for stop in stop_positions {
            let name = stop.tag("name").map(|s| s.to_string());
            let stop_ref = stop.tag("ref").map(|s| s.to_string());
            let lat = stop.latitude();
            let lon = stop.longitude();
            let transport_type = determine_transport_type(stop);

            // Check for missing coordinates
            let (lat, lon) = match (lat, lon) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => {
                    new_issues.push(OsmIssue::new(
                        stop.id,
                        &stop.element_type,
                        "stop_position",
                        OsmIssueType::MissingCoordinates,
                        transport_type.clone(),
                        format!("Stop position '{}' has no coordinates", name.as_deref().unwrap_or("unnamed")),
                        name,
                        stop_ref,
                        None,
                        None,
                    ));
                    continue;
                }
            };

            // Check for missing IFOPT
            if stop.tag("ref:IFOPT").is_none() {
                new_issues.push(OsmIssue::new(
                    stop.id,
                    &stop.element_type,
                    "stop_position",
                    OsmIssueType::MissingIfopt,
                    transport_type.clone(),
                    format!("Stop position '{}' has no ref:IFOPT tag", name.as_deref().unwrap_or("unnamed")),
                    name.clone(),
                    stop_ref.clone(),
                    Some(lat),
                    Some(lon),
                ));
            }

            // Check for missing name and ref (would show as "?" on map)
            if name.is_none() && stop_ref.is_none() {
                new_issues.push(OsmIssue::new(
                    stop.id,
                    &stop.element_type,
                    "stop_position",
                    OsmIssueType::MissingName,
                    transport_type,
                    "Stop position has no name or ref tag - displays as '?' on map".to_string(),
                    None,
                    None,
                    Some(lat),
                    Some(lon),
                ));
            }

            let tags_json = stop.tags.as_ref().and_then(|t| {
                serde_json::to_string(t)
                    .map_err(|e| tracing::warn!(osm_id = stop.id, error = %e, "Failed to serialize stop_position tags"))
                    .ok()
            });

            // Get station_id from stop_area membership
            let station_id = platform_station_map.get(&stop.id).copied();

            sqlx::query(
                r#"
                INSERT INTO stop_positions (osm_id, osm_type, name, ref, ref_ifopt, lat, lon, tags, station_id, area_id, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(osm_id) DO UPDATE SET
                    osm_type = excluded.osm_type,
                    name = excluded.name,
                    ref = excluded.ref,
                    ref_ifopt = excluded.ref_ifopt,
                    lat = excluded.lat,
                    lon = excluded.lon,
                    tags = excluded.tags,
                    station_id = COALESCE(excluded.station_id, stop_positions.station_id),
                    area_id = excluded.area_id,
                    updated_at = datetime('now')
                "#,
            )
            .bind(stop.id)
            .bind(&stop.element_type)
            .bind(stop.tag("name"))
            .bind(stop.tag("ref"))
            .bind(stop.tag("ref:IFOPT"))
            .bind(lat)
            .bind(lon)
            .bind(tags_json)
            .bind(station_id)
            .bind(area_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
        }

        // Store collected issues
        if !new_issues.is_empty() {
            let mut issues = self.issues.write().await;
            issues.extend(new_issues);
        }

        Ok(())
    }

    /// Store routes in database with ways and stops
    async fn store_routes(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        routes: &[OsmRoute],
        area_id: i64,
    ) -> Result<(), SyncError> {
        let mut new_issues = Vec::new();

        for route in routes {
            let transport_type = transport_type_from_route(&route.route_type);

            // Check for missing route ref (line number)
            if route.ref_number.is_none() {
                new_issues.push(OsmIssue::new(
                    route.osm_id,
                    &route.osm_type,
                    "route",
                    OsmIssueType::MissingRouteRef,
                    transport_type,
                    format!("Route '{}' has no ref (line number) tag", route.name.as_deref().unwrap_or("unnamed")),
                    route.name.clone(),
                    None, // ref_tag
                    None,
                    None,
                ));
            }

            let tags_json = serde_json::to_string(&route.tags)
                .map_err(|e| tracing::warn!(osm_id = route.osm_id, error = %e, "Failed to serialize route tags"))
                .ok();

            // Insert route
            sqlx::query(
                r#"
                INSERT INTO routes (osm_id, osm_type, name, ref, route_type, operator, network, color, tags, area_id, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(osm_id) DO UPDATE SET
                    osm_type = excluded.osm_type,
                    name = excluded.name,
                    ref = excluded.ref,
                    route_type = excluded.route_type,
                    operator = excluded.operator,
                    network = excluded.network,
                    color = excluded.color,
                    tags = excluded.tags,
                    area_id = excluded.area_id,
                    updated_at = datetime('now')
                "#,
            )
            .bind(route.osm_id)
            .bind(&route.osm_type)
            .bind(&route.name)
            .bind(&route.ref_number)
            .bind(&route.route_type)
            .bind(&route.operator)
            .bind(&route.network)
            .bind(&route.color)
            .bind(&tags_json)
            .bind(area_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

            // Delete existing ways and stops for this route
            sqlx::query("DELETE FROM route_ways WHERE route_id = ?")
                .bind(route.osm_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

            sqlx::query("DELETE FROM route_stops WHERE route_id = ?")
                .bind(route.osm_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

            // Insert ways
            for way in &route.ways {
                let geometry_json = serde_json::to_string(&way.geometry)
                    .map_err(|e| {
                        tracing::warn!(
                            route_id = route.osm_id,
                            way_id = way.way_osm_id,
                            error = %e,
                            "Failed to serialize way geometry"
                        )
                    })
                    .ok();

                sqlx::query(
                    r#"
                    INSERT INTO route_ways (route_id, way_osm_id, sequence, geometry)
                    VALUES (?, ?, ?, ?)
                    "#,
                )
                .bind(route.osm_id)
                .bind(way.way_osm_id)
                .bind(way.sequence)
                .bind(&geometry_json)
                .execute(&mut **tx)
                .await
                .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
            }

            // Insert stops - use subquery to only reference existing stop_positions (returns NULL if not found)
            for stop in &route.stops {
                sqlx::query(
                    r#"
                    INSERT INTO route_stops (route_id, stop_position_id, sequence, role)
                    VALUES (
                        ?,
                        (SELECT osm_id FROM stop_positions WHERE osm_id = ?),
                        ?,
                        ?
                    )
                    "#,
                )
                .bind(route.osm_id)
                .bind(stop.osm_id)
                .bind(stop.sequence)
                .bind(&stop.role)
                .execute(&mut **tx)
                .await
                .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
            }
        }

        // Store collected issues
        if !new_issues.is_empty() {
            let mut issues = self.issues.write().await;
            issues.extend(new_issues);
        }

        Ok(())
    }

    /// Resolve relations between features (platforms->stations, stop_positions->platforms, etc.)
    async fn resolve_relations(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        area_id: i64,
    ) -> Result<(), SyncError> {
        info!("Resolving relations for area {}", area_id);

        // Fetch all stations for distance calculations
        let stations: Vec<(i64, f64, f64)> = sqlx::query_as(
            "SELECT osm_id, lat, lon FROM stations WHERE area_id = ?",
        )
        .bind(area_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Link platforms to nearest station
        let platforms: Vec<(i64, f64, f64)> = sqlx::query_as(
            "SELECT osm_id, lat, lon FROM platforms WHERE area_id = ? AND station_id IS NULL",
        )
        .bind(area_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Max distance for fallback linking: ~500m ≈ 0.005 degrees
        let max_station_distance = 0.005_f64.powi(2);

        for (platform_id, plat, plon) in &platforms {
            // Find nearest station within max distance
            if let Some((station_id, _, _)) = stations
                .iter()
                .filter(|(_, slat, slon)| {
                    (plat - slat).powi(2) + (plon - slon).powi(2) < max_station_distance
                })
                .min_by(|a, b| {
                    let dist_a = (plat - a.1).powi(2) + (plon - a.2).powi(2);
                    let dist_b = (plat - b.1).powi(2) + (plon - b.2).powi(2);
                    // Use unwrap_or to handle NaN - treat NaN as greater
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Greater)
                })
            {
                sqlx::query("UPDATE platforms SET station_id = ? WHERE osm_id = ?")
                    .bind(station_id)
                    .bind(platform_id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
            }
        }

        // Fetch platforms with their coords for stop_position linking
        let platforms_with_coords: Vec<(i64, f64, f64)> = sqlx::query_as(
            "SELECT osm_id, lat, lon FROM platforms WHERE area_id = ?",
        )
        .bind(area_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Link stop_positions to nearest platform (within ~50m)
        let stop_positions: Vec<(i64, f64, f64)> = sqlx::query_as(
            "SELECT osm_id, lat, lon FROM stop_positions WHERE area_id = ? AND platform_id IS NULL",
        )
        .bind(area_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Threshold for stop_position to platform linking: ~50m ≈ 0.0005 degrees
        let platform_threshold = 0.0005_f64.powi(2);

        for (stop_id, slat, slon) in &stop_positions {
            if let Some((platform_id, _, _)) = platforms_with_coords
                .iter()
                .filter(|(_, plat, plon)| {
                    (slat - plat).powi(2) + (slon - plon).powi(2) < platform_threshold
                })
                .min_by(|a, b| {
                    let dist_a = (slat - a.1).powi(2) + (slon - a.2).powi(2);
                    let dist_b = (slat - b.1).powi(2) + (slon - b.2).powi(2);
                    // Use unwrap_or to handle NaN - treat NaN as greater
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Greater)
                })
            {
                sqlx::query("UPDATE stop_positions SET platform_id = ? WHERE osm_id = ?")
                    .bind(platform_id)
                    .bind(stop_id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| SyncError::DatabaseError(e.to_string()))?;
            }
        }

        // Link stop_positions to station via their platform
        sqlx::query(
            r#"
            UPDATE stop_positions
            SET station_id = (
                SELECT station_id FROM platforms WHERE osm_id = stop_positions.platform_id
            )
            WHERE area_id = ? AND station_id IS NULL AND platform_id IS NOT NULL
            "#,
        )
        .bind(area_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Resolve route_stops references from stop_positions
        sqlx::query(
            r#"
            UPDATE route_stops
            SET platform_id = (
                SELECT platform_id FROM stop_positions WHERE osm_id = route_stops.stop_position_id
            ),
            station_id = (
                SELECT station_id FROM stop_positions WHERE osm_id = route_stops.stop_position_id
            )
            WHERE route_id IN (SELECT osm_id FROM routes WHERE area_id = ?)
            "#,
        )
        .bind(area_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // For stops that reference platforms directly
        sqlx::query(
            r#"
            UPDATE route_stops
            SET platform_id = stop_position_id,
                station_id = (
                    SELECT station_id FROM platforms WHERE osm_id = route_stops.stop_position_id
                )
            WHERE route_id IN (SELECT osm_id FROM routes WHERE area_id = ?)
            AND platform_id IS NULL
            AND stop_position_id IN (SELECT osm_id FROM platforms)
            "#,
        )
        .bind(area_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        // Detect orphaned elements (still unlinked after fallback)
        let mut new_issues = Vec::new();

        // Find orphaned platforms (no station_id after all linking attempts)
        let orphaned_platforms: Vec<(i64, String, Option<String>, Option<String>, f64, f64)> = sqlx::query_as(
            "SELECT osm_id, osm_type, name, ref, lat, lon FROM platforms WHERE area_id = ? AND station_id IS NULL",
        )
        .bind(area_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        for (osm_id, osm_type, name, ref_tag, lat, lon) in orphaned_platforms {
            new_issues.push(OsmIssue::new(
                osm_id,
                &osm_type,
                "platform",
                OsmIssueType::OrphanedElement,
                TransportType::Unknown, // Transport type unknown for orphaned elements from DB query
                format!("Platform '{}' is not linked to any station (no stop_area relation and no station within 500m)", name.as_deref().unwrap_or("unnamed")),
                name,
                ref_tag,
                Some(lat),
                Some(lon),
            ));
        }

        // Find orphaned stop_positions (no station_id after all linking attempts)
        let orphaned_stops: Vec<(i64, String, Option<String>, Option<String>, f64, f64)> = sqlx::query_as(
            "SELECT osm_id, osm_type, name, ref, lat, lon FROM stop_positions WHERE area_id = ? AND station_id IS NULL",
        )
        .bind(area_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        for (osm_id, osm_type, name, ref_tag, lat, lon) in orphaned_stops {
            new_issues.push(OsmIssue::new(
                osm_id,
                &osm_type,
                "stop_position",
                OsmIssueType::OrphanedElement,
                TransportType::Unknown, // Transport type unknown for orphaned elements from DB query
                format!("Stop position '{}' is not linked to any station", name.as_deref().unwrap_or("unnamed")),
                name,
                ref_tag,
                Some(lat),
                Some(lon),
            ));
        }

        // Store collected issues
        if !new_issues.is_empty() {
            let mut issues = self.issues.write().await;
            issues.extend(new_issues);
        }

        info!("Finished resolving relations for area {}", area_id);
        Ok(())
    }

    /// Check for platforms without stop_positions and vice versa
    async fn check_platform_stop_pairs(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        area_id: i64,
    ) -> Result<(), SyncError> {
        let mut new_issues = Vec::new();

        // Distance threshold for nearby check: ~100m ≈ 0.001 degrees
        let nearby_threshold = 0.001;

        // Find platforms without any stop_position nearby (using coordinate check)
        let platforms_without_stops: Vec<(i64, String, Option<String>, Option<String>, Option<String>, f64, f64)> = sqlx::query_as(
            r#"
            SELECT p.osm_id, p.osm_type, p.name, p.ref, p.ref_ifopt, p.lat, p.lon
            FROM platforms p
            WHERE p.area_id = ?
            AND p.ref_ifopt IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM stop_positions sp
                WHERE sp.area_id = p.area_id
                AND ABS(sp.lat - p.lat) < ?
                AND ABS(sp.lon - p.lon) < ?
            )
            "#,
        )
        .bind(area_id)
        .bind(nearby_threshold)
        .bind(nearby_threshold)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        for (osm_id, osm_type, name, ref_tag, _ref_ifopt, lat, lon) in platforms_without_stops {
            new_issues.push(OsmIssue::new(
                osm_id,
                &osm_type,
                "platform",
                OsmIssueType::MissingStopPosition,
                TransportType::Unknown,
                format!("Platform '{}' has no stop_position nearby", name.as_deref().unwrap_or("unnamed")),
                name,
                ref_tag,
                Some(lat),
                Some(lon),
            ));
        }

        // Find stop_positions without any platform nearby (using coordinate check)
        let stops_without_platforms: Vec<(i64, String, Option<String>, Option<String>, Option<String>, f64, f64)> = sqlx::query_as(
            r#"
            SELECT sp.osm_id, sp.osm_type, sp.name, sp.ref, sp.ref_ifopt, sp.lat, sp.lon
            FROM stop_positions sp
            WHERE sp.area_id = ?
            AND sp.ref_ifopt IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM platforms p
                WHERE p.area_id = sp.area_id
                AND ABS(p.lat - sp.lat) < ?
                AND ABS(p.lon - sp.lon) < ?
            )
            "#,
        )
        .bind(area_id)
        .bind(nearby_threshold)
        .bind(nearby_threshold)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| SyncError::DatabaseError(e.to_string()))?;

        for (osm_id, osm_type, name, ref_tag, _ref_ifopt, lat, lon) in stops_without_platforms {
            new_issues.push(OsmIssue::new(
                osm_id,
                &osm_type,
                "stop_position",
                OsmIssueType::MissingPlatform,
                TransportType::Unknown,
                format!("Stop position '{}' has no platform nearby", name.as_deref().unwrap_or("unnamed")),
                name,
                ref_tag,
                Some(lat),
                Some(lon),
            ));
        }

        // Store collected issues
        if !new_issues.is_empty() {
            let mut issues = self.issues.write().await;
            issues.extend(new_issues);
        }

        info!("Checked platform/stop_position pairs for area {}", area_id);
        Ok(())
    }

    /// Sync departures for all stations
    async fn sync_all_departures(&self) {
        info!("Starting departure sync");

        // Get all unique stop IFOPTs from stations, platforms, and stop_positions
        let stop_ifopts: Vec<(String,)> = match sqlx::query_as(
            r#"
            SELECT DISTINCT ref_ifopt
            FROM stations
            WHERE ref_ifopt IS NOT NULL
            UNION
            SELECT DISTINCT ref_ifopt
            FROM platforms
            WHERE ref_ifopt IS NOT NULL
            UNION
            SELECT DISTINCT ref_ifopt
            FROM stop_positions
            WHERE ref_ifopt IS NOT NULL
            "#,
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                error!(error = %e, "Failed to fetch stop IFOPTs for departure sync");
                return;
            }
        };

        if stop_ifopts.is_empty() {
            warn!("No stop IFOPTs found for departure sync");
            return;
        }

        // Extract station-level IFOPTs (first 3 parts: de:09761:691)
        // EFA API works better with station-level IFOPTs and returns platform-level IFOPTs in response
        let station_ifopts: Vec<String> = stop_ifopts
            .into_iter()
            .map(|(ifopt,)| {
                // Take first 3 parts of IFOPT (de:XXXXX:NNN) to get station level
                let parts: Vec<&str> = ifopt.split(':').collect();
                if parts.len() >= 3 {
                    format!("{}:{}:{}", parts[0], parts[1], parts[2])
                } else {
                    ifopt
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Read sync config
        let efa_sync_config = {
            let config = self.config.read().await;
            config.efa_sync.clone()
        };
        let time_span = if efa_sync_config.time_span_minutes > 0 {
            Some(efa_sync_config.time_span_minutes)
        } else {
            None
        };

        info!(
            count = station_ifopts.len(),
            limit = efa_sync_config.limit_per_stop,
            time_span_minutes = ?time_span,
            fetch_arrivals = efa_sync_config.fetch_arrivals,
            "Fetching departures for stations"
        );

        // Fetch departures (always) and arrivals (if configured) concurrently
        let departure_results = self
            .efa_client
            .get_departures_batch(
                &station_ifopts,
                efa_sync_config.limit_per_stop,
                true,
                time_span,
            )
            .await;

        let arrival_results = if efa_sync_config.fetch_arrivals {
            self.efa_client
                .get_arrivals_batch(
                    &station_ifopts,
                    efa_sync_config.limit_per_stop,
                    true,
                    time_span,
                )
                .await
        } else {
            Vec::new()
        };

        let mut success_count = 0;
        let mut error_count = 0;
        let now = Utc::now();
        // Events older than this are considered expired and will be removed
        let expiry_cutoff = now - chrono::Duration::hours(2);

        // Collect new events by platform, deduplicating as we go
        // Key: (platform_ifopt, event_type), Value: HashMap<(line_number, planned_time), Departure>
        let mut new_departures_by_platform: HashMap<String, HashMap<(String, String), Departure>> =
            HashMap::new();
        let mut new_arrivals_by_platform: HashMap<String, HashMap<(String, String), Departure>> =
            HashMap::new();

        // Process departures - collect and deduplicate by (line_number, planned_time)
        for (station_ifopt, result) in departure_results {
            match result {
                Ok(response) => {
                    let departures =
                        self.parse_stop_events(&station_ifopt, &response.stop_events, now, EventType::Departure);

                    for departure in departures {
                        let platform_ifopt = departure.stop_ifopt.clone();
                        let key = (departure.line_number.clone(), departure.planned_time.clone());
                        new_departures_by_platform
                            .entry(platform_ifopt)
                            .or_default()
                            .insert(key, departure);
                    }
                    success_count += 1;
                }
                Err(e) => {
                    tracing::debug!(station = %station_ifopt, error = %e, "Failed to fetch departures, keeping existing data");
                    error_count += 1;
                }
            }
        }

        // Process arrivals - collect and deduplicate by (line_number, planned_time)
        for (station_ifopt, result) in arrival_results {
            match result {
                Ok(response) => {
                    let arrivals =
                        self.parse_stop_events(&station_ifopt, &response.stop_events, now, EventType::Arrival);

                    for arrival in arrivals {
                        let platform_ifopt = arrival.stop_ifopt.clone();
                        let key = (arrival.line_number.clone(), arrival.planned_time.clone());
                        new_arrivals_by_platform
                            .entry(platform_ifopt)
                            .or_default()
                            .insert(key, arrival);
                    }
                    success_count += 1;
                }
                Err(e) => {
                    tracing::debug!(station = %station_ifopt, error = %e, "Failed to fetch arrivals, keeping existing data");
                    error_count += 1;
                }
            }
        }

        // Now update the store atomically per platform
        let mut store = self.departures.write().await;

        // Update departures for platforms we fetched
        for (platform_ifopt, new_deps) in new_departures_by_platform {
            let entry = store.entry(platform_ifopt).or_default();
            // Remove old departures for this platform
            entry.retain(|d| d.event_type != EventType::Departure);
            // Add new deduplicated departures
            entry.extend(new_deps.into_values());
        }

        // Update arrivals for platforms we fetched
        for (platform_ifopt, new_arrs) in new_arrivals_by_platform {
            let entry = store.entry(platform_ifopt).or_default();
            // Remove old arrivals for this platform
            entry.retain(|d| d.event_type != EventType::Arrival);
            // Add new deduplicated arrivals
            entry.extend(new_arrs.into_values());
        }

        // Time-based expiration: remove events that are too old (more than 2 hours past)
        for events in store.values_mut() {
            events.retain(|event| {
                match DateTime::parse_from_rfc3339(&event.planned_time) {
                    Ok(event_time) => event_time > expiry_cutoff,
                    // Keep events with unparseable times (shouldn't happen, but defensive)
                    Err(_) => true,
                }
            });
        }

        // Clean up stops with no events
        store.retain(|_, events| !events.is_empty());

        // Sort events by time for each stop
        for events in store.values_mut() {
            events.sort_by(|a, b| a.planned_time.cmp(&b.planned_time));
        }

        // Release lock before logging
        drop(store);

        // Broadcast vehicle update notification to all WebSocket clients
        let update = VehicleUpdate {
            timestamp: Utc::now().to_rfc3339(),
            is_initial: false,
        };
        // Ignore send errors - they just mean no one is listening
        let _ = self.vehicle_updates_tx.send(update);

        info!(
            success = success_count,
            errors = error_count,
            "Completed departure/arrival sync"
        );
    }

    /// Parse stop events into Departure structs
    /// Returns departures keyed by their actual platform IFOPT from the EFA response
    fn parse_stop_events(
        &self,
        _station_ifopt: &str, // Station IFOPT we queried (kept for logging)
        stop_events: &[crate::providers::timetables::germany::bavaria::StopEvent],
        now: DateTime<Utc>,
        event_type: EventType,
    ) -> Vec<Departure> {
        let mut events = Vec::new();

        for event in stop_events {
            // Use the actual platform IFOPT from the event location
            let stop_ifopt = match event.location_ifopt() {
                Some(id) => id,
                None => continue, // Skip events without location ID
            };
            let line_number = match event.line_number() {
                Some(n) => n.to_string(),
                None => continue,
            };

            // For departures, use destination; for arrivals, use origin
            let destination = match event_type {
                EventType::Departure => match event.destination() {
                    Some(d) => d.to_string(),
                    None => continue,
                },
                EventType::Arrival => match event.origin() {
                    Some(o) => o.to_string(),
                    None => continue,
                },
            };

            // Get planned and estimated times based on event type
            let (planned, estimated) = match event_type {
                EventType::Departure => (
                    event.planned_departure().map(|s| s.to_string()),
                    event.estimated_departure().map(|s| s.to_string()),
                ),
                EventType::Arrival => (
                    event.planned_arrival().map(|s| s.to_string()),
                    event.estimated_arrival().map(|s| s.to_string()),
                ),
            };

            let planned = match planned {
                Some(p) => p,
                None => continue,
            };

            // Skip events in the past
            if let Ok(planned_dt) = DateTime::parse_from_rfc3339(&planned) {
                if planned_dt < now {
                    continue;
                }
            }

            let platform = event.platform().map(|s| s.to_string());

            // Calculate delay in minutes if we have both planned and estimated times
            let delay_minutes = match (&planned, &estimated) {
                (p, Some(e)) => {
                    if let (Ok(planned_dt), Ok(estimated_dt)) = (
                        DateTime::parse_from_rfc3339(p),
                        DateTime::parse_from_rfc3339(e),
                    ) {
                        Some(
                            (estimated_dt.signed_duration_since(planned_dt).num_seconds() / 60)
                                as i32,
                        )
                    } else {
                        None
                    }
                }
                _ => None,
            };

            // Get destination/origin ID based on event type
            let destination_id = match event_type {
                EventType::Departure => event.destination_id().map(|s| s.to_string()),
                EventType::Arrival => event.origin_id().map(|s| s.to_string()),
            };

            events.push(Departure {
                stop_ifopt: stop_ifopt.to_string(),
                event_type,
                line_number,
                destination,
                destination_id,
                planned_time: planned,
                estimated_time: estimated,
                delay_minutes,
                platform,
                trip_id: event.trip_id().map(|s| s.to_string()),
            });
        }

        events
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("OSM fetch error: {0}")]
    OsmError(String),
    #[error("EFA fetch error: {0}")]
    EfaError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
}
