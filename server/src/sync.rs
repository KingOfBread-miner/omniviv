use crate::config::{Area, Config};
use crate::providers::efa::EfaClient;
use crate::providers::osm::{OsmClient, OsmElement, OsmRoute};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use utoipa::ToSchema;

/// Type of stop event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Departure,
    Arrival,
}

/// A stop event (departure or arrival)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Departure {
    pub stop_ifopt: String,
    pub event_type: EventType,
    pub line_number: String,
    /// For departures: destination; for arrivals: origin
    pub destination: String,
    /// Destination stop ID (for departures) or origin stop ID (for arrivals)
    pub destination_id: Option<String>,
    pub planned_time: String,
    pub estimated_time: Option<String>,
    pub delay_minutes: Option<i32>,
    pub platform: Option<String>,
    /// Unique trip identifier (AVMSTripID) - consistent across all stops for a journey
    pub trip_id: Option<String>,
}

// Keep backward compatibility with old field names
impl Departure {
    pub fn planned_departure(&self) -> &str {
        &self.planned_time
    }

    pub fn estimated_departure(&self) -> Option<&str> {
        self.estimated_time.as_deref()
    }
}

/// In-memory store for departure data
pub type DepartureStore = Arc<RwLock<HashMap<String, Vec<Departure>>>>;

/// Manages background synchronization of OSM and EFA data
pub struct SyncManager {
    pool: SqlitePool,
    osm_client: OsmClient,
    efa_client: EfaClient,
    config: Arc<RwLock<Config>>,
    departures: DepartureStore,
}

impl SyncManager {
    pub fn new(pool: SqlitePool, config: Config) -> Result<Self, SyncError> {
        let osm_client = OsmClient::new().map_err(|e| SyncError::OsmError(e.to_string()))?;
        let efa_client = EfaClient::new().map_err(|e| SyncError::EfaError(e.to_string()))?;

        Ok(Self {
            pool,
            osm_client,
            efa_client,
            config: Arc::new(RwLock::new(config)),
            departures: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get a reference to the departure store for API access
    pub fn departure_store(&self) -> DepartureStore {
        self.departures.clone()
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

        // Spawn departure sync loop (every 30 seconds)
        let efa_self = self.clone();
        let efa_handle = tokio::spawn(async move {
            // Wait a bit for initial OSM sync to complete
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

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
        for station in stations {
            let (lat, lon) = match (station.latitude(), station.longitude()) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => continue,
            };

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
        for platform in platforms {
            let (lat, lon) = match (platform.latitude(), platform.longitude()) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => continue,
            };

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
        for stop in stop_positions {
            let (lat, lon) = match (stop.latitude(), stop.longitude()) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => continue,
            };

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

        Ok(())
    }

    /// Store routes in database with ways and stops
    async fn store_routes(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        routes: &[OsmRoute],
        area_id: i64,
    ) -> Result<(), SyncError> {
        for route in routes {
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

        info!("Finished resolving relations for area {}", area_id);
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

        info!(count = station_ifopts.len(), "Fetching departures and arrivals for stations");

        // Fetch departures and arrivals concurrently
        let (departure_results, arrival_results) = tokio::join!(
            self.efa_client.get_departures_batch(&station_ifopts, 10, true),
            self.efa_client.get_arrivals_batch(&station_ifopts, 10, true)
        );

        let mut success_count = 0;
        let mut error_count = 0;
        let now = Utc::now();

        // Update the store incrementally - only update stops that had successful fetches
        // This preserves existing data for stops that failed and avoids full HashMap replacement
        let mut store = self.departures.write().await;

        // Track which platform IFOPTs we've updated for departures/arrivals
        let mut updated_departure_platforms: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut updated_arrival_platforms: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Process departures - group by actual platform IFOPT from the response
        for (station_ifopt, result) in departure_results {
            match result {
                Ok(response) => {
                    let departures =
                        self.parse_stop_events(&station_ifopt, &response.stop_events, now, EventType::Departure);

                    // Group departures by their actual platform IFOPT
                    for departure in departures {
                        let platform_ifopt = departure.stop_ifopt.clone();
                        updated_departure_platforms.insert(platform_ifopt.clone());
                        let entry = store.entry(platform_ifopt).or_insert_with(Vec::new);
                        entry.push(departure);
                    }
                    success_count += 1;
                }
                Err(e) => {
                    tracing::debug!(station = %station_ifopt, error = %e, "Failed to fetch departures, keeping existing data");
                    error_count += 1;
                }
            }
        }

        // Remove old departures for platforms we updated
        for platform_ifopt in &updated_departure_platforms {
            if let Some(entry) = store.get_mut(platform_ifopt) {
                // Keep only arrivals and newly added departures (added in this sync)
                let new_departures: Vec<_> = entry.iter()
                    .filter(|d| d.event_type == EventType::Departure)
                    .cloned()
                    .collect();
                entry.retain(|d| d.event_type != EventType::Departure);
                // Deduplicate by keeping unique (line_number, planned_time) combinations
                let mut seen = std::collections::HashSet::new();
                for dep in new_departures {
                    let key = (dep.line_number.clone(), dep.planned_time.clone());
                    if seen.insert(key) {
                        entry.push(dep);
                    }
                }
            }
        }

        // Process arrivals - group by actual platform IFOPT from the response
        for (station_ifopt, result) in arrival_results {
            match result {
                Ok(response) => {
                    let arrivals =
                        self.parse_stop_events(&station_ifopt, &response.stop_events, now, EventType::Arrival);

                    // Group arrivals by their actual platform IFOPT
                    for arrival in arrivals {
                        let platform_ifopt = arrival.stop_ifopt.clone();
                        updated_arrival_platforms.insert(platform_ifopt.clone());
                        let entry = store.entry(platform_ifopt).or_insert_with(Vec::new);
                        entry.push(arrival);
                    }
                    success_count += 1;
                }
                Err(e) => {
                    tracing::debug!(station = %station_ifopt, error = %e, "Failed to fetch arrivals, keeping existing data");
                    error_count += 1;
                }
            }
        }

        // Remove old arrivals for platforms we updated
        for platform_ifopt in &updated_arrival_platforms {
            if let Some(entry) = store.get_mut(platform_ifopt) {
                let new_arrivals: Vec<_> = entry.iter()
                    .filter(|d| d.event_type == EventType::Arrival)
                    .cloned()
                    .collect();
                entry.retain(|d| d.event_type != EventType::Arrival);
                // Deduplicate by keeping unique (line_number, planned_time) combinations
                let mut seen = std::collections::HashSet::new();
                for arr in new_arrivals {
                    let key = (arr.line_number.clone(), arr.planned_time.clone());
                    if seen.insert(key) {
                        entry.push(arr);
                    }
                }
            }
        }

        // Clean up stops with no events
        store.retain(|_, events| !events.is_empty());

        // Sort events by time for each stop
        for events in store.values_mut() {
            events.sort_by(|a, b| a.planned_time.cmp(&b.planned_time));
        }

        // Release lock before logging
        drop(store);

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
        stop_events: &[crate::providers::efa::StopEvent],
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
