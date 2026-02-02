use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate, Weekday};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use super::error::GtfsError;

/// Maximum allowed download size for GTFS zip (500 MB)
const MAX_DOWNLOAD_SIZE: u64 = 500 * 1024 * 1024;
/// Maximum allowed total decompressed size for GTFS zip (2 GB)
const MAX_DECOMPRESSED_SIZE: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum length for cached HTTP header values (ETag, Last-Modified)
const MAX_HEADER_LENGTH: usize = 1024;

// --- Public types for the in-memory schedule ---

/// A GTFS stop (from stops.txt).
///
/// Some fields (e.g. `parent_station`) are parsed from the feed but not
/// directly read in the current codebase. They are retained for debugging,
/// future use (e.g. parent-child stop grouping), and completeness of the
/// in-memory GTFS model.
#[derive(Debug, Clone)]
pub struct GtfsStop {
    pub stop_id: String,
    pub stop_name: Option<String>,
    /// Used for IFOPT mapping: leaf stops have a parent_station.
    pub parent_station: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

/// A GTFS route (from routes.txt).
///
/// Fields like `route_id`, `route_long_name`, and `route_type` are parsed
/// for completeness and future use (e.g. filtering by route type). Currently
/// `route_short_name` is the primary field used for line number display.
#[derive(Debug, Clone)]
pub struct GtfsRoute {
    pub route_id: String,
    pub route_short_name: Option<String>,
    pub route_long_name: Option<String>,
    pub route_type: Option<i32>,
}

/// A GTFS trip (from trips.txt).
///
/// `trip_id` and `direction_id` are parsed for completeness and used as
/// HashMap keys and for potential future direction-based filtering.
#[derive(Debug, Clone)]
pub struct GtfsTrip {
    pub trip_id: String,
    pub route_id: String,
    pub service_id: String,
    pub trip_headsign: Option<String>,
    pub direction_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct GtfsStopTime {
    pub stop_sequence: i32,
    pub stop_id: String,
    /// Seconds since midnight (can exceed 86400 for trips crossing midnight)
    pub arrival_time: Option<i32>,
    /// Seconds since midnight
    pub departure_time: Option<i32>,
}

/// A GTFS calendar entry (from calendar.txt).
///
/// `service_id` is stored alongside the HashMap key for self-contained
/// debug printing and test construction.
#[derive(Debug, Clone)]
pub struct GtfsCalendar {
    pub service_id: String,
    pub days: [bool; 7], // mon, tue, wed, thu, fri, sat, sun
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug, Clone)]
pub struct GtfsCalendarDate {
    pub date: NaiveDate,
    /// 1 = service added, 2 = service removed
    pub exception_type: i32,
}

/// The full in-memory GTFS schedule.
///
/// `loaded_at` tracks when the schedule was parsed, used by the health
/// endpoint and for cache freshness logging.
pub struct GtfsSchedule {
    pub stops: HashMap<String, GtfsStop>,
    pub routes: HashMap<String, GtfsRoute>,
    pub trips: HashMap<String, GtfsTrip>,
    /// trip_id -> ordered stop_times
    pub stop_times: HashMap<String, Vec<GtfsStopTime>>,
    pub calendars: HashMap<String, GtfsCalendar>,
    /// service_id -> list of exceptions
    pub calendar_dates: HashMap<String, Vec<GtfsCalendarDate>>,
    /// GTFS stop_id -> set of trip_ids visiting that stop (for fast filtering)
    pub trips_by_stop: HashMap<String, HashSet<String>>,
    /// IFOPT -> list of matching GTFS stop_ids (built after loading via spatial matching)
    pub ifopt_to_gtfs: HashMap<String, Vec<String>>,
    /// GTFS stop_id -> IFOPT (reverse mapping)
    pub gtfs_to_ifopt: HashMap<String, String>,
    pub loaded_at: chrono::DateTime<chrono::Utc>,
}

impl GtfsSchedule {
    /// Check if a service is active on the given date.
    pub fn is_service_active(&self, service_id: &str, date: NaiveDate) -> bool {
        // Check calendar_dates exceptions first (they override regular calendar)
        if let Some(exceptions) = self.calendar_dates.get(service_id) {
            for exc in exceptions {
                if exc.date == date {
                    return exc.exception_type == 1;
                }
            }
        }

        // Check regular calendar
        if let Some(cal) = self.calendars.get(service_id) {
            if date < cal.start_date || date > cal.end_date {
                return false;
            }
            let day_index = match date.weekday() {
                Weekday::Mon => 0,
                Weekday::Tue => 1,
                Weekday::Wed => 2,
                Weekday::Thu => 3,
                Weekday::Fri => 4,
                Weekday::Sat => 5,
                Weekday::Sun => 6,
            };
            return cal.days[day_index];
        }

        // If only calendar_dates exist (no calendar entry), service is active
        // only on dates explicitly listed with exception_type=1.
        // We already checked above and found no matching date, so inactive.
        false
    }

    /// Get the last stop_id of a trip (useful for destination_id).
    /// Returns IFOPT if a mapping exists, otherwise the raw GTFS stop_id.
    pub fn last_stop_of_trip(&self, trip_id: &str) -> Option<String> {
        let last_stop = self.stop_times.get(trip_id)?.last()?;
        Some(
            self.gtfs_to_ifopt
                .get(&last_stop.stop_id)
                .cloned()
                .unwrap_or_else(|| last_stop.stop_id.clone()),
        )
    }

    /// Build the IFOPT <-> GTFS stop ID mapping using spatial matching.
    ///
    /// For each provided IFOPT (with lat/lon), finds the nearest GTFS stop
    /// within `max_distance_m` meters. Only matches leaf stops (those with
    /// a parent_station, i.e. the stops actually used in stop_times).
    pub fn build_ifopt_mapping(&mut self, db_stops: &[(String, f64, f64)], max_distance_m: f64) {
        self.ifopt_to_gtfs.clear();
        self.gtfs_to_ifopt.clear();

        // Collect leaf GTFS stops (those that appear in stop_times or have a parent_station)
        // with coordinates
        let gtfs_leaf_stops: Vec<(&str, f64, f64)> = self
            .stops
            .values()
            .filter(|s| {
                // Leaf stops: have a parent_station OR appear in trips_by_stop
                (s.parent_station.is_some() || self.trips_by_stop.contains_key(&s.stop_id))
                    && s.lat.is_some()
                    && s.lon.is_some()
            })
            .map(|s| (s.stop_id.as_str(), s.lat.unwrap(), s.lon.unwrap()))
            .collect();

        // Convert max distance from meters to approximate degrees (rough approximation at ~48° lat)
        let max_dist_deg = max_distance_m / 111_000.0;
        let max_dist_sq = max_dist_deg * max_dist_deg;

        let mut matched = 0usize;

        for (ifopt, db_lat, db_lon) in db_stops {
            let mut best: Option<(&str, f64)> = None;

            for &(gtfs_id, glat, glon) in &gtfs_leaf_stops {
                let dlat = db_lat - glat;
                // Adjust longitude distance for latitude
                let dlon = (db_lon - glon) * (db_lat.to_radians().cos());
                let dist_sq = dlat * dlat + dlon * dlon;

                if dist_sq < max_dist_sq
                    && (best.is_none() || dist_sq < best.unwrap().1)
                {
                    best = Some((gtfs_id, dist_sq));
                }
            }

            if let Some((gtfs_id, _)) = best {
                self.ifopt_to_gtfs
                    .entry(ifopt.clone())
                    .or_default()
                    .push(gtfs_id.to_string());
                // Only set reverse mapping if not already claimed by a closer match
                self.gtfs_to_ifopt
                    .entry(gtfs_id.to_string())
                    .or_insert_with(|| ifopt.clone());
                matched += 1;
            }
        }

        info!(
            db_stops = db_stops.len(),
            gtfs_leaf_stops = gtfs_leaf_stops.len(),
            matched,
            "Built IFOPT <-> GTFS stop mapping"
        );
    }

    /// Look up trip_ids for an IFOPT via the mapping.
    /// Returns trips that visit any GTFS stop mapped to this IFOPT.
    pub fn trips_for_ifopt(&self, ifopt: &str) -> HashSet<&String> {
        let mut result = HashSet::new();
        if let Some(gtfs_ids) = self.ifopt_to_gtfs.get(ifopt) {
            for gid in gtfs_ids {
                if let Some(trips) = self.trips_by_stop.get(gid) {
                    result.extend(trips);
                }
            }
        }
        result
    }

    /// Check if a GTFS stop_id maps to any of the given IFOPTs.
    pub fn is_gtfs_stop_relevant(&self, gtfs_stop_id: &str, ifopt_set: &HashSet<String>) -> bool {
        if let Some(ifopt) = self.gtfs_to_ifopt.get(gtfs_stop_id) {
            ifopt_set.contains(ifopt)
        } else {
            false
        }
    }

    /// Get the IFOPT for a GTFS stop_id, falling back to the raw stop_id.
    pub fn ifopt_for_gtfs_stop(&self, gtfs_stop_id: &str) -> String {
        self.gtfs_to_ifopt
            .get(gtfs_stop_id)
            .cloned()
            .unwrap_or_else(|| gtfs_stop_id.to_string())
    }
}

// --- Download and loading ---

/// Known files in the cache directory. Everything else is cleaned up.
const CACHE_KNOWN_FILES: &[&str] = &["latest.zip", "metadata.json"];

/// Remove unexpected files from the cache directory and log disk usage.
async fn cleanup_cache(cache_dir: &Path) {
    let mut total_size: u64 = 0;
    let mut removed = 0usize;

    let mut entries = match tokio::fs::read_dir(cache_dir).await {
        Ok(entries) => entries,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if let Ok(meta) = entry.metadata().await {
            if CACHE_KNOWN_FILES.contains(&name.as_ref()) {
                total_size += meta.len();
            } else if meta.is_file() {
                // Remove unknown files (e.g., stale temp files from interrupted downloads)
                if let Err(e) = tokio::fs::remove_file(entry.path()).await {
                    warn!(file = %name, error = %e, "Failed to clean up unknown cache file");
                } else {
                    info!(file = %name, size_bytes = meta.len(), "Removed unknown file from GTFS cache");
                    removed += 1;
                }
            }
        }
    }

    if removed > 0 {
        info!(removed, "Cleaned up GTFS cache directory");
    }
    debug!(total_size_mb = total_size / (1024 * 1024), "GTFS cache disk usage");
}

/// Download the static GTFS feed to the cache directory.
pub async fn download_feed(
    client: &reqwest::Client,
    url: &str,
    cache_dir: &str,
) -> Result<PathBuf, GtfsError> {
    let cache_path = Path::new(cache_dir);
    tokio::fs::create_dir_all(cache_path).await?;

    // Clean up stale/unknown files before downloading
    cleanup_cache(cache_path).await;

    let zip_path = cache_path.join("latest.zip");
    let metadata_path = cache_path.join("metadata.json");

    // Conditional request with ETag/Last-Modified
    let mut request = client.get(url);
    if let Ok(meta_content) = tokio::fs::read_to_string(&metadata_path).await {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_content) {
            if let Some(etag) = meta.get("etag").and_then(|v| v.as_str()) {
                request = request.header("If-None-Match", etag);
            }
            if let Some(last_modified) = meta.get("last_modified").and_then(|v| v.as_str()) {
                request = request.header("If-Modified-Since", last_modified);
            }
        }
    }

    let response = request
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        info!("Static GTFS feed not modified, using cached version");
        return Ok(zip_path);
    }

    if !response.status().is_success() {
        return Err(GtfsError::NetworkMessage(format!(
            "GTFS download HTTP {}",
            response.status()
        )));
    }

    // Check Content-Length before downloading
    if let Some(content_length) = response.content_length() {
        if content_length > MAX_DOWNLOAD_SIZE {
            return Err(GtfsError::NetworkMessage(format!(
                "GTFS download too large: {} bytes (max {} bytes)",
                content_length, MAX_DOWNLOAD_SIZE
            )));
        }
    }

    // Save headers for future conditional requests (limited to MAX_HEADER_LENGTH)
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .filter(|s| s.len() <= MAX_HEADER_LENGTH)
        .map(|s| s.to_string());
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|v| v.to_str().ok())
        .filter(|s| s.len() <= MAX_HEADER_LENGTH)
        .map(|s| s.to_string());

    // Stream download with size limit
    let mut total_bytes: u64 = 0;
    let mut file = tokio::fs::File::create(&zip_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total_bytes += chunk.len() as u64;
        if total_bytes > MAX_DOWNLOAD_SIZE {
            drop(file);
            let _ = tokio::fs::remove_file(&zip_path).await;
            return Err(GtfsError::NetworkMessage(format!(
                "GTFS download exceeded size limit at {} bytes (max {} bytes)",
                total_bytes, MAX_DOWNLOAD_SIZE
            )));
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    info!(size_mb = total_bytes / (1024 * 1024), "Downloaded static GTFS feed");

    let meta = serde_json::json!({
        "etag": etag,
        "last_modified": last_modified,
        "downloaded_at": chrono::Utc::now().to_rfc3339(),
    });
    let _ = tokio::fs::write(&metadata_path, meta.to_string()).await;

    Ok(zip_path)
}

/// Load the GTFS zip into an in-memory schedule (blocking — call on spawn_blocking).
pub fn load_schedule(zip_path: &Path) -> Result<GtfsSchedule, GtfsError> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // ZIP bomb protection: check total uncompressed size
    let mut total_uncompressed: u64 = 0;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            total_uncompressed += entry.size();
        }
    }
    if total_uncompressed > MAX_DECOMPRESSED_SIZE {
        return Err(GtfsError::ParseError(format!(
            "GTFS zip decompressed size {} bytes exceeds limit {} bytes",
            total_uncompressed, MAX_DECOMPRESSED_SIZE
        )));
    }
    info!(
        compressed_mb = std::fs::metadata(zip_path).map(|m| m.len() / (1024 * 1024)).unwrap_or(0),
        decompressed_mb = total_uncompressed / (1024 * 1024),
        "Verified GTFS zip size within limits"
    );

    let stops = parse_stops(&mut archive)?;
    info!(count = stops.len(), "Parsed GTFS stops");

    let routes = parse_routes(&mut archive)?;
    info!(count = routes.len(), "Parsed GTFS routes");

    let trips = parse_trips(&mut archive)?;
    info!(count = trips.len(), "Parsed GTFS trips");

    let stop_times = parse_stop_times(&mut archive)?;
    let total_st: usize = stop_times.values().map(|v| v.len()).sum();
    info!(trips_with_times = stop_times.len(), total_stop_times = total_st, "Parsed GTFS stop_times");

    let calendars = parse_calendar(&mut archive);
    info!(count = calendars.len(), "Parsed GTFS calendar");

    let calendar_dates = parse_calendar_dates(&mut archive);
    let total_cd: usize = calendar_dates.values().map(|v| v.len()).sum();
    info!(services = calendar_dates.len(), total_exceptions = total_cd, "Parsed GTFS calendar_dates");

    // Build reverse index: stop_id -> trip_ids
    let mut trips_by_stop: HashMap<String, HashSet<String>> = HashMap::new();
    for (trip_id, sts) in &stop_times {
        for st in sts {
            trips_by_stop
                .entry(st.stop_id.clone())
                .or_default()
                .insert(trip_id.clone());
        }
    }
    info!(stops_indexed = trips_by_stop.len(), "Built trips-by-stop index");

    Ok(GtfsSchedule {
        stops,
        routes,
        trips,
        stop_times,
        calendars,
        calendar_dates,
        trips_by_stop,
        ifopt_to_gtfs: HashMap::new(),
        gtfs_to_ifopt: HashMap::new(),
        loaded_at: chrono::Utc::now(),
    })
}

// --- Helper functions ---

/// Extract station-level IFOPT (first 3 colon-separated parts).
/// e.g., "de:09761:691:0:a" -> "de:09761:691"
pub fn station_level_ifopt(ifopt: &str) -> String {
    let parts: Vec<&str> = ifopt.split(':').collect();
    if parts.len() >= 3 {
        format!("{}:{}:{}", parts[0], parts[1], parts[2])
    } else {
        ifopt.to_string()
    }
}

/// Extract platform identifier from IFOPT (5th part).
/// e.g., "de:09761:691:0:a" -> Some("a")
pub fn extract_platform_from_ifopt(ifopt: &str) -> Option<String> {
    let parts: Vec<&str> = ifopt.split(':').collect();
    if parts.len() >= 5 {
        Some(parts[4].to_string())
    } else {
        None
    }
}

/// Parse GTFS time string "HH:MM:SS" to seconds since midnight.
/// Supports hours >= 24 for trips crossing midnight.
pub fn parse_gtfs_time(time_str: &str) -> Option<i32> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: i32 = parts[0].parse().ok()?;
    let minutes: i32 = parts[1].parse().ok()?;
    let seconds: i32 = parts[2].parse().ok()?;
    Some(hours * 3600 + minutes * 60 + seconds)
}

/// Parse GTFS date string "YYYYMMDD" to NaiveDate.
fn parse_gtfs_date(s: &str) -> Option<NaiveDate> {
    if s.len() != 8 {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[4..6].parse().ok()?;
    let day: u32 = s[6..8].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

// --- CSV parsing ---

fn parse_stops(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<HashMap<String, GtfsStop>, GtfsError> {
    info!("Parsing stops.txt");
    let file = archive.by_name("stops.txt")?;
    let mut rdr = csv::Reader::from_reader(file);
    let headers = rdr.headers()?.clone();

    let idx_id = headers
        .iter()
        .position(|h| h == "stop_id")
        .ok_or_else(|| GtfsError::ParseError("stops.txt missing stop_id".into()))?;
    let idx_name = headers.iter().position(|h| h == "stop_name");
    let idx_parent = headers.iter().position(|h| h == "parent_station");
    let idx_lat = headers.iter().position(|h| h == "stop_lat");
    let idx_lon = headers.iter().position(|h| h == "stop_lon");

    let mut stops = HashMap::new();
    let mut skipped = 0usize;
    for result in rdr.records() {
        let record = result?;
        let stop_id = record.get(idx_id).unwrap_or("").to_string();
        if stop_id.is_empty() {
            skipped += 1;
            continue;
        }
        stops.insert(
            stop_id.clone(),
            GtfsStop {
                stop_id,
                stop_name: idx_name.and_then(|i| record.get(i)).and_then(non_empty),
                parent_station: idx_parent
                    .and_then(|i| record.get(i))
                    .and_then(non_empty),
                lat: idx_lat
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                lon: idx_lon
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
            },
        );
    }
    if skipped > 0 {
        warn!(skipped, "Skipped stops.txt records with empty stop_id");
    }
    Ok(stops)
}

fn parse_routes(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<HashMap<String, GtfsRoute>, GtfsError> {
    info!("Parsing routes.txt");
    let file = archive.by_name("routes.txt")?;
    let mut rdr = csv::Reader::from_reader(file);
    let headers = rdr.headers()?.clone();

    let idx_id = headers
        .iter()
        .position(|h| h == "route_id")
        .ok_or_else(|| GtfsError::ParseError("routes.txt missing route_id".into()))?;
    let idx_short = headers.iter().position(|h| h == "route_short_name");
    let idx_long = headers.iter().position(|h| h == "route_long_name");
    let idx_type = headers.iter().position(|h| h == "route_type");

    let mut routes = HashMap::new();
    let mut skipped = 0usize;
    for result in rdr.records() {
        let record = result?;
        let route_id = record.get(idx_id).unwrap_or("").to_string();
        if route_id.is_empty() {
            skipped += 1;
            continue;
        }
        routes.insert(
            route_id.clone(),
            GtfsRoute {
                route_id,
                route_short_name: idx_short
                    .and_then(|i| record.get(i))
                    .and_then(non_empty),
                route_long_name: idx_long
                    .and_then(|i| record.get(i))
                    .and_then(non_empty),
                route_type: idx_type
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
            },
        );
    }
    if skipped > 0 {
        warn!(skipped, "Skipped routes.txt records with empty route_id");
    }
    Ok(routes)
}

fn parse_trips(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<HashMap<String, GtfsTrip>, GtfsError> {
    info!("Parsing trips.txt");
    let file = archive.by_name("trips.txt")?;
    let mut rdr = csv::Reader::from_reader(file);
    let headers = rdr.headers()?.clone();

    let idx_trip = headers
        .iter()
        .position(|h| h == "trip_id")
        .ok_or_else(|| GtfsError::ParseError("trips.txt missing trip_id".into()))?;
    let idx_route = headers
        .iter()
        .position(|h| h == "route_id")
        .ok_or_else(|| GtfsError::ParseError("trips.txt missing route_id".into()))?;
    let idx_service = headers
        .iter()
        .position(|h| h == "service_id")
        .ok_or_else(|| GtfsError::ParseError("trips.txt missing service_id".into()))?;
    let idx_headsign = headers.iter().position(|h| h == "trip_headsign");
    let idx_dir = headers.iter().position(|h| h == "direction_id");

    let mut trips = HashMap::new();
    let mut skipped = 0usize;
    for result in rdr.records() {
        let record = result?;
        let trip_id = record.get(idx_trip).unwrap_or("").to_string();
        if trip_id.is_empty() {
            skipped += 1;
            continue;
        }
        trips.insert(
            trip_id.clone(),
            GtfsTrip {
                trip_id,
                route_id: record.get(idx_route).unwrap_or("").to_string(),
                service_id: record.get(idx_service).unwrap_or("").to_string(),
                trip_headsign: idx_headsign
                    .and_then(|i| record.get(i))
                    .and_then(non_empty),
                direction_id: idx_dir
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
            },
        );
    }
    if skipped > 0 {
        warn!(skipped, "Skipped trips.txt records with empty trip_id");
    }
    Ok(trips)
}

fn parse_stop_times(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<HashMap<String, Vec<GtfsStopTime>>, GtfsError> {
    info!("Parsing stop_times.txt");
    let file = archive.by_name("stop_times.txt")?;
    let mut rdr = csv::Reader::from_reader(file);
    let headers = rdr.headers()?.clone();

    let idx_trip = headers
        .iter()
        .position(|h| h == "trip_id")
        .ok_or_else(|| GtfsError::ParseError("stop_times.txt missing trip_id".into()))?;
    let idx_seq = headers
        .iter()
        .position(|h| h == "stop_sequence")
        .ok_or_else(|| GtfsError::ParseError("stop_times.txt missing stop_sequence".into()))?;
    let idx_stop = headers
        .iter()
        .position(|h| h == "stop_id")
        .ok_or_else(|| GtfsError::ParseError("stop_times.txt missing stop_id".into()))?;
    let idx_arr = headers.iter().position(|h| h == "arrival_time");
    let idx_dep = headers.iter().position(|h| h == "departure_time");

    let mut stop_times: HashMap<String, Vec<GtfsStopTime>> = HashMap::new();
    let mut skipped = 0usize;
    for result in rdr.records() {
        let record = result?;
        let trip_id = record.get(idx_trip).unwrap_or("").to_string();
        if trip_id.is_empty() {
            skipped += 1;
            continue;
        }
        let st = GtfsStopTime {
            stop_sequence: record
                .get(idx_seq)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            stop_id: record.get(idx_stop).unwrap_or("").to_string(),
            arrival_time: idx_arr
                .and_then(|i| record.get(i))
                .and_then(parse_gtfs_time),
            departure_time: idx_dep
                .and_then(|i| record.get(i))
                .and_then(parse_gtfs_time),
        };
        stop_times.entry(trip_id).or_default().push(st);
    }
    if skipped > 0 {
        warn!(skipped, "Skipped stop_times.txt records with empty trip_id");
    }

    // Sort each trip's stop_times by stop_sequence
    for sts in stop_times.values_mut() {
        sts.sort_by_key(|st| st.stop_sequence);
    }

    Ok(stop_times)
}

fn parse_calendar(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> HashMap<String, GtfsCalendar> {
    info!("Parsing calendar.txt");
    let file = match archive.by_name("calendar.txt") {
        Ok(f) => f,
        Err(_) => {
            info!("No calendar.txt in GTFS zip (optional file)");
            return HashMap::new();
        }
    };
    let mut rdr = csv::Reader::from_reader(file);
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return HashMap::new(),
    };

    let idx_service = headers.iter().position(|h| h == "service_id");
    let idx_mon = headers.iter().position(|h| h == "monday");
    let idx_tue = headers.iter().position(|h| h == "tuesday");
    let idx_wed = headers.iter().position(|h| h == "wednesday");
    let idx_thu = headers.iter().position(|h| h == "thursday");
    let idx_fri = headers.iter().position(|h| h == "friday");
    let idx_sat = headers.iter().position(|h| h == "saturday");
    let idx_sun = headers.iter().position(|h| h == "sunday");
    let idx_start = headers.iter().position(|h| h == "start_date");
    let idx_end = headers.iter().position(|h| h == "end_date");

    let Some(idx_service) = idx_service else {
        return HashMap::new();
    };

    let mut calendars = HashMap::new();
    let mut skipped = 0usize;
    for result in rdr.records() {
        let Ok(record) = result else {
            skipped += 1;
            continue;
        };
        let service_id = record.get(idx_service).unwrap_or("").to_string();
        if service_id.is_empty() {
            skipped += 1;
            continue;
        }

        let get_bool = |idx: Option<usize>| -> bool {
            idx.and_then(|i| record.get(i))
                .and_then(|s| s.parse::<i32>().ok())
                .map(|v| v == 1)
                .unwrap_or(false)
        };

        let start_date = idx_start
            .and_then(|i| record.get(i))
            .and_then(parse_gtfs_date);
        let end_date = idx_end
            .and_then(|i| record.get(i))
            .and_then(parse_gtfs_date);

        let (Some(start_date), Some(end_date)) = (start_date, end_date) else {
            skipped += 1;
            continue;
        };

        calendars.insert(
            service_id.clone(),
            GtfsCalendar {
                service_id,
                days: [
                    get_bool(idx_mon),
                    get_bool(idx_tue),
                    get_bool(idx_wed),
                    get_bool(idx_thu),
                    get_bool(idx_fri),
                    get_bool(idx_sat),
                    get_bool(idx_sun),
                ],
                start_date,
                end_date,
            },
        );
    }
    if skipped > 0 {
        warn!(skipped, "Skipped calendar.txt records (empty/unparseable)");
    }
    calendars
}

fn parse_calendar_dates(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> HashMap<String, Vec<GtfsCalendarDate>> {
    info!("Parsing calendar_dates.txt");
    let file = match archive.by_name("calendar_dates.txt") {
        Ok(f) => f,
        Err(_) => {
            info!("No calendar_dates.txt in GTFS zip (optional file)");
            return HashMap::new();
        }
    };
    let mut rdr = csv::Reader::from_reader(file);
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return HashMap::new(),
    };

    let idx_service = headers.iter().position(|h| h == "service_id");
    let idx_date = headers.iter().position(|h| h == "date");
    let idx_type = headers.iter().position(|h| h == "exception_type");

    let (Some(idx_service), Some(idx_date), Some(idx_type)) = (idx_service, idx_date, idx_type)
    else {
        return HashMap::new();
    };

    let mut dates: HashMap<String, Vec<GtfsCalendarDate>> = HashMap::new();
    let mut skipped = 0usize;
    for result in rdr.records() {
        let Ok(record) = result else {
            skipped += 1;
            continue;
        };
        let service_id = record.get(idx_service).unwrap_or("").to_string();
        if service_id.is_empty() {
            skipped += 1;
            continue;
        }
        let Some(date) = record.get(idx_date).and_then(parse_gtfs_date) else {
            skipped += 1;
            continue;
        };
        let exception_type = record
            .get(idx_type)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        dates.entry(service_id).or_default().push(GtfsCalendarDate {
            date,
            exception_type,
        });
    }
    if skipped > 0 {
        warn!(skipped, "Skipped calendar_dates.txt records (empty/unparseable)");
    }
    dates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gtfs_time() {
        assert_eq!(parse_gtfs_time("08:30:00"), Some(30600));
        assert_eq!(parse_gtfs_time("00:00:00"), Some(0));
        assert_eq!(parse_gtfs_time("24:00:00"), Some(86400));
        assert_eq!(parse_gtfs_time("25:30:00"), Some(91800));
        assert_eq!(parse_gtfs_time("invalid"), None);
        assert_eq!(parse_gtfs_time(""), None);
    }

    #[test]
    fn test_parse_gtfs_date() {
        assert_eq!(
            parse_gtfs_date("20260201"),
            Some(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap())
        );
        assert_eq!(parse_gtfs_date("invalid"), None);
        assert_eq!(parse_gtfs_date(""), None);
    }

    #[test]
    fn test_station_level_ifopt() {
        assert_eq!(station_level_ifopt("de:09761:691:0:a"), "de:09761:691");
        assert_eq!(station_level_ifopt("de:09761:691"), "de:09761:691");
        assert_eq!(station_level_ifopt("de:09761:691:0"), "de:09761:691");
        assert_eq!(station_level_ifopt("short"), "short");
    }

    #[test]
    fn test_extract_platform_from_ifopt() {
        assert_eq!(
            extract_platform_from_ifopt("de:09761:691:0:a"),
            Some("a".to_string())
        );
        assert_eq!(extract_platform_from_ifopt("de:09761:691:0"), None);
        assert_eq!(extract_platform_from_ifopt("de:09761:691"), None);
    }

    #[test]
    fn test_is_service_active() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        // Monday 2026-02-02
        let monday = NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();
        // Saturday 2026-02-07
        let saturday = NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();

        // Service runs Mon-Fri
        schedule.calendars.insert(
            "weekday".into(),
            GtfsCalendar {
                service_id: "weekday".into(),
                days: [true, true, true, true, true, false, false],
                start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
        );

        assert!(schedule.is_service_active("weekday", monday));
        assert!(!schedule.is_service_active("weekday", saturday));

        // Exception: add service on a Saturday
        schedule
            .calendar_dates
            .insert("weekday".into(), vec![GtfsCalendarDate {
                date: saturday,
                exception_type: 1,
            }]);
        assert!(schedule.is_service_active("weekday", saturday));

        // Unknown service
        assert!(!schedule.is_service_active("unknown", monday));
    }

    #[test]
    fn test_is_service_active_exception_type_2_removes_service() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        let monday = NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();

        // Regular weekday service
        schedule.calendars.insert(
            "weekday".into(),
            GtfsCalendar {
                service_id: "weekday".into(),
                days: [true, true, true, true, true, false, false],
                start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
        );

        assert!(schedule.is_service_active("weekday", monday));

        // Exception type 2: remove service on this Monday (e.g., holiday)
        schedule.calendar_dates.insert(
            "weekday".into(),
            vec![GtfsCalendarDate {
                date: monday,
                exception_type: 2,
            }],
        );

        assert!(!schedule.is_service_active("weekday", monday));
    }

    #[test]
    fn test_is_service_active_before_start_date() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        // Service starts in the future
        schedule.calendars.insert(
            "future".into(),
            GtfsCalendar {
                service_id: "future".into(),
                days: [true; 7],
                start_date: NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2027, 12, 31).unwrap(),
            },
        );

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        assert!(!schedule.is_service_active("future", today));
    }

    #[test]
    fn test_is_service_active_after_end_date() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        // Service ended in the past
        schedule.calendars.insert(
            "past".into(),
            GtfsCalendar {
                service_id: "past".into(),
                days: [true; 7],
                start_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            },
        );

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        assert!(!schedule.is_service_active("past", today));
    }

    #[test]
    fn test_is_service_active_calendar_dates_only() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        // Some GTFS feeds use only calendar_dates without calendar.txt
        let special_day = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap();
        let normal_day = NaiveDate::from_ymd_opt(2026, 12, 26).unwrap();

        schedule.calendar_dates.insert(
            "holiday_only".into(),
            vec![GtfsCalendarDate {
                date: special_day,
                exception_type: 1,
            }],
        );

        assert!(schedule.is_service_active("holiday_only", special_day));
        assert!(!schedule.is_service_active("holiday_only", normal_day));
    }

    #[test]
    fn test_parse_gtfs_time_edge_cases() {
        assert_eq!(parse_gtfs_time("23:59:59"), Some(86399));
        assert_eq!(parse_gtfs_time("48:00:00"), Some(172800));
        assert_eq!(parse_gtfs_time("00:00:01"), Some(1));
        // Invalid formats
        assert_eq!(parse_gtfs_time("8:30:00"), Some(30600)); // single digit hours still parse
        assert_eq!(parse_gtfs_time("08:30"), None); // missing seconds
        assert_eq!(parse_gtfs_time("08:30:00:00"), None); // too many parts
    }

    #[test]
    fn test_parse_gtfs_date_edge_cases() {
        assert_eq!(parse_gtfs_date("20260229"), None); // 2026 is not leap year
        assert_eq!(parse_gtfs_date("20240229"), Some(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap())); // 2024 is leap year
        assert_eq!(parse_gtfs_date("20260101"), Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()));
        assert_eq!(parse_gtfs_date("20261231"), Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()));
        assert_eq!(parse_gtfs_date("00000101"), Some(NaiveDate::from_ymd_opt(0, 1, 1).unwrap()));
    }

    #[test]
    fn test_station_level_ifopt_empty() {
        assert_eq!(station_level_ifopt(""), "");
        assert_eq!(station_level_ifopt("a"), "a");
        assert_eq!(station_level_ifopt("a:b"), "a:b");
    }

    #[test]
    fn test_extract_platform_from_ifopt_various() {
        assert_eq!(extract_platform_from_ifopt(""), None);
        assert_eq!(extract_platform_from_ifopt("a:b:c:d:e"), Some("e".to_string()));
        assert_eq!(
            extract_platform_from_ifopt("de:09761:691:0:Gleis 1"),
            Some("Gleis 1".to_string())
        );
        // Exactly 5 parts
        assert_eq!(
            extract_platform_from_ifopt("a:b:c:d:e"),
            Some("e".to_string())
        );
        // More than 5 parts - still returns 5th
        assert_eq!(
            extract_platform_from_ifopt("a:b:c:d:e:f"),
            Some("e".to_string())
        );
    }

    #[test]
    fn test_non_empty() {
        assert_eq!(non_empty("hello"), Some("hello".to_string()));
        assert_eq!(non_empty(""), None);
        assert_eq!(non_empty(" "), Some(" ".to_string())); // whitespace is not empty
    }

    #[test]
    fn test_last_stop_of_trip() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        schedule.stop_times.insert(
            "trip1".to_string(),
            vec![
                GtfsStopTime {
                    stop_sequence: 1,
                    stop_id: "stop_A".to_string(),
                    arrival_time: Some(28800),
                    departure_time: Some(28800),
                },
                GtfsStopTime {
                    stop_sequence: 2,
                    stop_id: "stop_B".to_string(),
                    arrival_time: Some(29700),
                    departure_time: None,
                },
            ],
        );

        // Without IFOPT mapping, returns raw stop_id
        assert_eq!(schedule.last_stop_of_trip("trip1"), Some("stop_B".to_string()));

        // With IFOPT mapping, returns IFOPT
        schedule.gtfs_to_ifopt.insert("stop_B".to_string(), "de:09761:691".to_string());
        assert_eq!(schedule.last_stop_of_trip("trip1"), Some("de:09761:691".to_string()));

        // Unknown trip returns None
        assert_eq!(schedule.last_stop_of_trip("nonexistent"), None);
    }

    #[test]
    fn test_build_ifopt_mapping() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        // Add GTFS stops with coordinates
        schedule.stops.insert(
            "1001".to_string(),
            GtfsStop {
                stop_id: "1001".to_string(),
                stop_name: Some("Test Stop".to_string()),
                parent_station: Some("100".to_string()),
                lat: Some(48.3705),
                lon: Some(10.8978),
            },
        );

        // Add the stop to trips_by_stop so it counts as a leaf
        schedule.trips_by_stop.insert(
            "1001".to_string(),
            std::iter::once("trip1".to_string()).collect(),
        );

        // DB stops with IFOPT and coordinates very close to GTFS stop
        let db_stops = vec![
            ("de:09761:691:0:1".to_string(), 48.3706, 10.8979),
        ];

        schedule.build_ifopt_mapping(&db_stops, 200.0);

        assert!(schedule.ifopt_to_gtfs.contains_key("de:09761:691:0:1"));
        assert_eq!(
            schedule.gtfs_to_ifopt.get("1001"),
            Some(&"de:09761:691:0:1".to_string())
        );
    }

    #[test]
    fn test_build_ifopt_mapping_no_match_beyond_distance() {
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        schedule.stops.insert(
            "far_stop".to_string(),
            GtfsStop {
                stop_id: "far_stop".to_string(),
                stop_name: Some("Far Stop".to_string()),
                parent_station: Some("parent".to_string()),
                lat: Some(49.0),  // ~70km away
                lon: Some(11.0),
            },
        );

        schedule.trips_by_stop.insert(
            "far_stop".to_string(),
            std::iter::once("trip1".to_string()).collect(),
        );

        let db_stops = vec![
            ("de:09761:691:0:1".to_string(), 48.37, 10.89),
        ];

        schedule.build_ifopt_mapping(&db_stops, 200.0); // 200m max

        assert!(schedule.ifopt_to_gtfs.is_empty());
        assert!(schedule.gtfs_to_ifopt.is_empty());
    }

    #[test]
    fn test_stop_times_sorted_with_gaps_in_sequence() {
        // Verify that stop_times with non-contiguous sequence numbers sort correctly
        let mut schedule = GtfsSchedule {
            stops: HashMap::new(),
            routes: HashMap::new(),
            trips: HashMap::new(),
            stop_times: HashMap::new(),
            calendars: HashMap::new(),
            calendar_dates: HashMap::new(),
            trips_by_stop: HashMap::new(),
            ifopt_to_gtfs: HashMap::new(),
            gtfs_to_ifopt: HashMap::new(),
            loaded_at: chrono::Utc::now(),
        };

        // Insert stop_times out of order with gaps in sequence
        schedule.stop_times.insert(
            "trip_gap".to_string(),
            vec![
                GtfsStopTime {
                    stop_sequence: 10,
                    stop_id: "stop_C".to_string(),
                    arrival_time: Some(30600),
                    departure_time: Some(30600),
                },
                GtfsStopTime {
                    stop_sequence: 1,
                    stop_id: "stop_A".to_string(),
                    arrival_time: Some(28800),
                    departure_time: Some(28800),
                },
                GtfsStopTime {
                    stop_sequence: 5,
                    stop_id: "stop_B".to_string(),
                    arrival_time: Some(29700),
                    departure_time: Some(29700),
                },
            ],
        );

        // Sort like load_schedule does
        for sts in schedule.stop_times.values_mut() {
            sts.sort_by_key(|st| st.stop_sequence);
        }

        let times = &schedule.stop_times["trip_gap"];
        assert_eq!(times[0].stop_sequence, 1);
        assert_eq!(times[0].stop_id, "stop_A");
        assert_eq!(times[1].stop_sequence, 5);
        assert_eq!(times[1].stop_id, "stop_B");
        assert_eq!(times[2].stop_sequence, 10);
        assert_eq!(times[2].stop_id, "stop_C");

        // last_stop_of_trip should return the highest sequence stop
        assert_eq!(schedule.last_stop_of_trip("trip_gap"), Some("stop_C".to_string()));
    }

    #[test]
    fn test_stop_times_duplicate_sequence_numbers() {
        // Duplicate sequence numbers shouldn't crash — they'll be adjacent after sort
        let mut stop_times: HashMap<String, Vec<GtfsStopTime>> = HashMap::new();
        stop_times.insert(
            "trip_dup".to_string(),
            vec![
                GtfsStopTime {
                    stop_sequence: 1,
                    stop_id: "stop_A".to_string(),
                    arrival_time: Some(28800),
                    departure_time: Some(28800),
                },
                GtfsStopTime {
                    stop_sequence: 1, // duplicate
                    stop_id: "stop_B".to_string(),
                    arrival_time: Some(29000),
                    departure_time: Some(29000),
                },
                GtfsStopTime {
                    stop_sequence: 2,
                    stop_id: "stop_C".to_string(),
                    arrival_time: Some(29700),
                    departure_time: Some(29700),
                },
            ],
        );

        for sts in stop_times.values_mut() {
            sts.sort_by_key(|st| st.stop_sequence);
        }

        let times = &stop_times["trip_dup"];
        assert_eq!(times.len(), 3);
        assert_eq!(times[0].stop_sequence, 1);
        assert_eq!(times[1].stop_sequence, 1);
        assert_eq!(times[2].stop_sequence, 2);
    }
}
