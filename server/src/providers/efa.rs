use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;

const EFA_BASE_URL: &str = "https://bahnland-bayern.de/efa/XML_DM_REQUEST";
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
}

impl EfaClient {
    pub fn new() -> Result<Self, EfaError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| EfaError::NetworkError(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            client,
            rate_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
        })
    }

    /// Fetch stop events (departures or arrivals) for a stop by its IFOPT ID
    async fn get_stop_events(
        &self,
        stop_ifopt: &str,
        limit: u32,
        tram_only: bool,
        event_type: StopEventType,
    ) -> Result<DepartureResponse, EfaError> {
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

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| EfaError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(EfaError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| EfaError::NetworkError(e.to_string()))?;

        serde_json::from_str(&body).map_err(|e| {
            tracing::warn!(
                "Failed to parse EFA response for {}: {} - body: {}",
                stop_ifopt,
                e,
                &body[..body.len().min(500)]
            );
            EfaError::ParseError(e.to_string())
        })
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
