use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{broadcast, Semaphore};
use uuid::Uuid;

use crate::sync::EfaRequestLog;

const EFA_BASE_URL: &str = "https://bahnland-bayern.de/efa/XML_DM_REQUEST";
const EFA_COORD_URL: &str = "https://bahnland-bayern.de/efa/XML_COORD_REQUEST";
/// Maximum concurrent requests to EFA API to avoid overwhelming the service
const MAX_CONCURRENT_REQUESTS: usize = 10;

/// Type of stop event (departure or arrival)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopEventType {
    Departure,
    Arrival,
}

#[derive(Debug, Error)]
pub enum EfaError {
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("API error: {0}")]
    ApiError(String),
}

/// EFA API client for fetching real-time departure data
pub struct EfaClient {
    client: Client,
    /// Semaphore to limit concurrent requests
    rate_limiter: Arc<Semaphore>,
    /// Sender for request diagnostics
    diagnostics_tx: broadcast::Sender<EfaRequestLog>,
}

impl EfaClient {
    pub fn new(diagnostics_tx: broadcast::Sender<EfaRequestLog>) -> Result<Self, EfaError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| EfaError::NetworkError(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            rate_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            diagnostics_tx,
        })
    }

    /// Send a diagnostics log entry
    fn log_request(&self, log: EfaRequestLog) {
        // Ignore send errors - they just mean no one is listening
        let _ = self.diagnostics_tx.send(log);
    }

    /// Fetch stop events (departures or arrivals) for a stop by its IFOPT ID
    async fn get_stop_events(
        &self,
        stop_ifopt: &str,
        limit: u32,
        tram_only: bool,
        event_type: StopEventType,
    ) -> Result<DepartureResponse, EfaError> {
        let start = Instant::now();
        let request_id = Uuid::new_v4().to_string();
        let endpoint = match event_type {
            StopEventType::Departure => "XML_DM_REQUEST (departures)",
            StopEventType::Arrival => "XML_DM_REQUEST (arrivals)",
        };

        // Build params for logging
        let mut params = HashMap::new();
        params.insert("stop_ifopt".to_string(), stop_ifopt.to_string());
        params.insert("limit".to_string(), limit.to_string());
        params.insert("tram_only".to_string(), tram_only.to_string());

        let mut url = format!(
            "{}?mode=direct&name_dm={}&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&limit={}&useRealtime=1",
            EFA_BASE_URL,
            urlencoding::encode(stop_ifopt),
            limit
        );

        if tram_only {
            url.push_str("&includedMeans=4");
        }

        // Add arrival/departure filter
        match event_type {
            StopEventType::Departure => url.push_str("&itdDateTimeDepArr=dep"),
            StopEventType::Arrival => url.push_str("&itdDateTimeDepArr=arr"),
        }

        let response = match self.client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status: 0,
                    response_size: None,
                    error: Some(e.to_string()),
                });
                return Err(EfaError::NetworkError(e.to_string()));
            }
        };

        let status = response.status().as_u16();

        if !response.status().is_success() {
            self.log_request(EfaRequestLog {
                id: request_id,
                timestamp: Utc::now().to_rfc3339(),
                method: "GET".to_string(),
                endpoint: endpoint.to_string(),
                params: Some(params),
                duration_ms: start.elapsed().as_millis() as u64,
                status,
                response_size: None,
                error: Some(format!("HTTP error: {}", status)),
            });
            return Err(EfaError::ApiError(format!("HTTP error: {}", status)));
        }

        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => {
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status,
                    response_size: None,
                    error: Some(format!("Failed to read body: {}", e)),
                });
                return Err(EfaError::NetworkError(e.to_string()));
            }
        };

        let response_size = body.len();

        let result: Result<DepartureResponse, _> = serde_json::from_str(&body);

        match &result {
            Ok(_) => {
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status,
                    response_size: Some(response_size),
                    error: None,
                });
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse EFA response for {}: {} - body: {}",
                    stop_ifopt,
                    e,
                    &body[..body.len().min(500)]
                );
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status,
                    response_size: Some(response_size),
                    error: Some(format!("Parse error: {}", e)),
                });
            }
        }

        result.map_err(|e| EfaError::ParseError(e.to_string()))
    }

    /// Fetch departures for a stop by its IFOPT ID (e.g., "de:09761:101")
    pub async fn get_departures(
        &self,
        stop_ifopt: &str,
        limit: u32,
        tram_only: bool,
    ) -> Result<DepartureResponse, EfaError> {
        self.get_stop_events(stop_ifopt, limit, tram_only, StopEventType::Departure)
            .await
    }

    /// Fetch arrivals for a stop by its IFOPT ID (e.g., "de:09761:101")
    pub async fn get_arrivals(
        &self,
        stop_ifopt: &str,
        limit: u32,
        tram_only: bool,
    ) -> Result<DepartureResponse, EfaError> {
        self.get_stop_events(stop_ifopt, limit, tram_only, StopEventType::Arrival)
            .await
    }

    /// Fetch stop events for multiple stops concurrently with rate limiting
    pub async fn get_stop_events_batch(
        &self,
        stop_ifopts: &[String],
        limit_per_stop: u32,
        tram_only: bool,
        event_type: StopEventType,
    ) -> Vec<(String, Result<DepartureResponse, EfaError>)> {
        let semaphore = self.rate_limiter.clone();

        let futures: Vec<_> = stop_ifopts
            .iter()
            .map(|ifopt| {
                let ifopt = ifopt.clone();
                let sem = semaphore.clone();
                async move {
                    // Acquire permit before making request (limits concurrent requests)
                    let _permit = sem.acquire().await.expect("Semaphore closed unexpectedly");
                    let result = self
                        .get_stop_events(&ifopt, limit_per_stop, tram_only, event_type)
                        .await;
                    (ifopt, result)
                }
            })
            .collect();

        futures::future::join_all(futures).await
    }

    /// Fetch departures for multiple stops concurrently with rate limiting
    pub async fn get_departures_batch(
        &self,
        stop_ifopts: &[String],
        limit_per_stop: u32,
        tram_only: bool,
    ) -> Vec<(String, Result<DepartureResponse, EfaError>)> {
        self.get_stop_events_batch(stop_ifopts, limit_per_stop, tram_only, StopEventType::Departure)
            .await
    }

    /// Fetch arrivals for multiple stops concurrently with rate limiting
    pub async fn get_arrivals_batch(
        &self,
        stop_ifopts: &[String],
        limit_per_stop: u32,
        tram_only: bool,
    ) -> Vec<(String, Result<DepartureResponse, EfaError>)> {
        self.get_stop_events_batch(stop_ifopts, limit_per_stop, tram_only, StopEventType::Arrival)
            .await
    }

    /// Search for stops near given coordinates
    /// Returns stops within the specified radius (in meters)
    pub async fn find_stops_by_coord(
        &self,
        lon: f64,
        lat: f64,
        radius_meters: u32,
    ) -> Result<CoordSearchResponse, EfaError> {
        let start = Instant::now();
        let request_id = Uuid::new_v4().to_string();
        let endpoint = "XML_COORD_REQUEST";

        let mut params = HashMap::new();
        params.insert("lon".to_string(), lon.to_string());
        params.insert("lat".to_string(), lat.to_string());
        params.insert("radius".to_string(), radius_meters.to_string());

        let url = format!(
            "{}?outputFormat=rapidJSON&coord={}:{}:WGS84&inclFilter=1&type_1=STOP&radius_1={}",
            EFA_COORD_URL, lon, lat, radius_meters
        );

        let response = match self.client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status: 0,
                    response_size: None,
                    error: Some(e.to_string()),
                });
                return Err(EfaError::NetworkError(e.to_string()));
            }
        };

        let status = response.status().as_u16();

        if !response.status().is_success() {
            self.log_request(EfaRequestLog {
                id: request_id,
                timestamp: Utc::now().to_rfc3339(),
                method: "GET".to_string(),
                endpoint: endpoint.to_string(),
                params: Some(params),
                duration_ms: start.elapsed().as_millis() as u64,
                status,
                response_size: None,
                error: Some(format!("HTTP error: {}", status)),
            });
            return Err(EfaError::ApiError(format!("HTTP error: {}", status)));
        }

        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => {
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status,
                    response_size: None,
                    error: Some(format!("Failed to read body: {}", e)),
                });
                return Err(EfaError::NetworkError(e.to_string()));
            }
        };

        let response_size = body.len();
        let result: Result<CoordSearchResponse, _> = serde_json::from_str(&body);

        match &result {
            Ok(_) => {
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status,
                    response_size: Some(response_size),
                    error: None,
                });
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse EFA coord response: {} - body: {}",
                    e,
                    &body[..body.len().min(500)]
                );
                self.log_request(EfaRequestLog {
                    id: request_id,
                    timestamp: Utc::now().to_rfc3339(),
                    method: "GET".to_string(),
                    endpoint: endpoint.to_string(),
                    params: Some(params),
                    duration_ms: start.elapsed().as_millis() as u64,
                    status,
                    response_size: Some(response_size),
                    error: Some(format!("Parse error: {}", e)),
                });
            }
        }

        result.map_err(|e| EfaError::ParseError(e.to_string()))
    }

    /// Get all platforms for a station by querying departures
    /// Returns a list of unique platforms with their full IFOPTs
    pub async fn get_station_platforms(
        &self,
        station_ifopt: &str,
    ) -> Result<Vec<PlatformInfo>, EfaError> {
        // Query departures for this station to get platform information
        let response = self.get_stop_events(station_ifopt, 20, false, StopEventType::Departure).await?;

        let mut platforms: std::collections::HashMap<String, PlatformInfo> = std::collections::HashMap::new();

        for event in &response.stop_events {
            if let Some(location) = &event.location {
                if let Some(id) = &location.id {
                    // Only include platform-level IFOPTs (more than 3 parts)
                    if id.split(':').count() > 3 && !platforms.contains_key(id) {
                        let platform = location.properties.as_ref()
                            .and_then(|p| p.platform.clone());
                        let name = location.disassembled_name.clone()
                            .or_else(|| location.properties.as_ref()?.platform_name.clone());
                        let station_name = location.parent.as_ref()
                            .and_then(|p| p.name.clone())
                            .or_else(|| location.name.clone());

                        platforms.insert(id.clone(), PlatformInfo {
                            ifopt: id.clone(),
                            platform,
                            name,
                            station_name,
                        });
                    }
                }
            }
        }

        Ok(platforms.into_values().collect())
    }
}

// Response structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartureResponse {
    pub version: Option<String>,
    #[serde(default)]
    pub locations: Vec<Location>,
    #[serde(default, rename = "stopEvents")]
    pub stop_events: Vec<StopEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "disassembledName")]
    pub disassembled_name: Option<String>,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub coord: Option<Vec<f64>>,
    pub properties: Option<LocationProperties>,
    pub parent: Option<LocationParent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationParent {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub parent_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationProperties {
    #[serde(rename = "stopId")]
    pub stop_id: Option<String>,
    pub area: Option<String>,
    pub platform: Option<String>,
    #[serde(rename = "platformName")]
    pub platform_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopEvent {
    pub location: Option<Location>,
    #[serde(rename = "departureTimePlanned")]
    pub departure_time_planned: Option<String>,
    #[serde(rename = "departureTimeEstimated")]
    pub departure_time_estimated: Option<String>,
    #[serde(rename = "arrivalTimePlanned")]
    pub arrival_time_planned: Option<String>,
    #[serde(rename = "arrivalTimeEstimated")]
    pub arrival_time_estimated: Option<String>,
    pub transportation: Option<Transportation>,
    #[serde(default)]
    pub infos: Vec<Info>,
    /// Properties containing trip identifiers
    pub properties: Option<StopEventProperties>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopEventProperties {
    /// Unique trip identifier consistent across all stops for a journey
    #[serde(rename = "AVMSTripID")]
    pub avms_trip_id: Option<String>,
    /// Trip code (may be null at some stops)
    #[serde(rename = "tripCode")]
    pub trip_code: Option<i32>,
}

impl StopEvent {
    /// Get the best available departure time (estimated if available, otherwise planned)
    pub fn departure_time(&self) -> Option<&str> {
        self.departure_time_estimated
            .as_deref()
            .or(self.departure_time_planned.as_deref())
    }

    /// Get the planned departure time
    pub fn planned_departure(&self) -> Option<&str> {
        self.departure_time_planned.as_deref()
    }

    /// Get the estimated departure time (real-time)
    pub fn estimated_departure(&self) -> Option<&str> {
        self.departure_time_estimated.as_deref()
    }

    /// Get the best available arrival time (estimated if available, otherwise planned)
    pub fn arrival_time(&self) -> Option<&str> {
        self.arrival_time_estimated
            .as_deref()
            .or(self.arrival_time_planned.as_deref())
    }

    /// Get the planned arrival time
    pub fn planned_arrival(&self) -> Option<&str> {
        self.arrival_time_planned.as_deref()
    }

    /// Get the estimated arrival time (real-time)
    pub fn estimated_arrival(&self) -> Option<&str> {
        self.arrival_time_estimated.as_deref()
    }

    /// Get the line number (e.g., "1", "2", "3")
    pub fn line_number(&self) -> Option<&str> {
        self.transportation.as_ref()?.number.as_deref()
    }

    /// Get the destination name
    pub fn destination(&self) -> Option<&str> {
        self.transportation
            .as_ref()?
            .destination
            .as_ref()?
            .name
            .as_deref()
    }

    /// Get the origin name
    pub fn origin(&self) -> Option<&str> {
        self.transportation
            .as_ref()?
            .origin
            .as_ref()?
            .name
            .as_deref()
    }

    /// Get the platform identifier (e.g., "A1", "B2")
    pub fn platform(&self) -> Option<&str> {
        self.location
            .as_ref()?
            .properties
            .as_ref()?
            .platform
            .as_deref()
    }

    /// Get the unique trip ID (AVMSTripID) that identifies this vehicle journey
    pub fn trip_id(&self) -> Option<&str> {
        self.properties.as_ref()?.avms_trip_id.as_deref()
    }

    /// Get the destination stop ID
    pub fn destination_id(&self) -> Option<&str> {
        self.transportation
            .as_ref()?
            .destination
            .as_ref()?
            .id
            .as_deref()
    }

    /// Get the location/platform IFOPT (e.g., "de:09761:691:0:a")
    pub fn location_ifopt(&self) -> Option<&str> {
        self.location.as_ref()?.id.as_deref()
    }

    /// Get the origin stop ID
    pub fn origin_id(&self) -> Option<&str> {
        self.transportation
            .as_ref()?
            .origin
            .as_ref()?
            .id
            .as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transportation {
    pub id: Option<String>,
    pub name: Option<String>,
    pub number: Option<String>,
    pub product: Option<Product>,
    pub destination: Option<Destination>,
    pub origin: Option<Destination>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Option<i32>,
    pub class: Option<i32>,
    pub name: Option<String>,
    #[serde(rename = "iconId")]
    pub icon_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Destination {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub destination_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub priority: Option<String>,
    pub id: Option<String>,
    pub version: Option<i32>,
    #[serde(rename = "type")]
    pub info_type: Option<String>,
    #[serde(default, rename = "infoLinks")]
    pub info_links: Vec<InfoLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoLink {
    #[serde(rename = "urlText")]
    pub url_text: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
    pub subtitle: Option<String>,
}

// Coordinate search response structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordSearchResponse {
    pub version: Option<String>,
    #[serde(default)]
    pub locations: Vec<CoordLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordLocation {
    /// IFOPT ID (e.g., "de:09761:105")
    pub id: Option<String>,
    /// Stop name (e.g., "Staatstheater")
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    /// Coordinates in EFA format
    pub coord: Option<Vec<f64>>,
    pub parent: Option<CoordLocationParent>,
    pub properties: Option<CoordLocationProperties>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordLocationParent {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub parent_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordLocationProperties {
    /// Distance in meters from the queried coordinates
    pub distance: Option<u32>,
    #[serde(rename = "STOP_GLOBAL_ID")]
    pub stop_global_id: Option<String>,
    #[serde(rename = "STOP_NAME_WITH_PLACE")]
    pub stop_name_with_place: Option<String>,
}

impl CoordLocation {
    /// Get the IFOPT ID
    pub fn ifopt(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Get the distance in meters from the queried coordinates
    pub fn distance_meters(&self) -> Option<u32> {
        self.properties.as_ref()?.distance
    }

    /// Get the full name with place (e.g., "Augsburg, Staatstheater")
    pub fn full_name(&self) -> Option<&str> {
        self.properties
            .as_ref()?
            .stop_name_with_place
            .as_deref()
            .or(self.name.as_deref())
    }
}

/// Platform information extracted from departure data
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Full platform IFOPT (e.g., "de:09761:691:0:a")
    pub ifopt: String,
    /// Platform letter/number (e.g., "a", "1")
    pub platform: Option<String>,
    /// Platform name (e.g., "Bstg. a")
    pub name: Option<String>,
    /// Parent station name
    pub station_name: Option<String>,
}
