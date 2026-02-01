use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub areas: Vec<Area>,
    /// Allowed CORS origins. Required unless cors_permissive is true.
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Explicitly allow all origins (development only). Defaults to false.
    #[serde(default)]
    pub cors_permissive: bool,
    /// EFA API sync configuration
    #[serde(default)]
    pub efa_sync: EfaSyncConfig,
}

/// Configuration for EFA API departure sync
#[derive(Debug, Clone, Deserialize)]
pub struct EfaSyncConfig {
    /// Interval in seconds between departure sync cycles (default: 60)
    #[serde(default = "EfaSyncConfig::default_interval_secs")]
    pub interval_secs: u64,
    /// Maximum number of departures/arrivals to fetch per stop (default: 30)
    #[serde(default = "EfaSyncConfig::default_limit_per_stop")]
    pub limit_per_stop: u32,
    /// Time span in minutes to query departures for (default: 60)
    /// A larger time span means each request returns more events,
    /// reducing the need for frequent polling.
    #[serde(default = "EfaSyncConfig::default_time_span_minutes")]
    pub time_span_minutes: u32,
    /// Maximum concurrent requests to the EFA API (default: 10)
    #[serde(default = "EfaSyncConfig::default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    /// Whether to also fetch arrivals in addition to departures (default: true)
    /// Disabling this halves the number of API requests.
    #[serde(default = "EfaSyncConfig::default_fetch_arrivals")]
    pub fetch_arrivals: bool,
}

impl Default for EfaSyncConfig {
    fn default() -> Self {
        Self {
            interval_secs: Self::default_interval_secs(),
            limit_per_stop: Self::default_limit_per_stop(),
            time_span_minutes: Self::default_time_span_minutes(),
            max_concurrent_requests: Self::default_max_concurrent_requests(),
            fetch_arrivals: Self::default_fetch_arrivals(),
        }
    }
}

impl EfaSyncConfig {
    fn default_interval_secs() -> u64 {
        60
    }
    fn default_limit_per_stop() -> u32 {
        30
    }
    fn default_time_span_minutes() -> u32 {
        60
    }
    fn default_max_concurrent_requests() -> usize {
        10
    }
    fn default_fetch_arrivals() -> bool {
        true
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Area {
    pub name: String,
    pub bounding_box: BoundingBox,
    pub transport_types: Vec<TransportType>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct BoundingBox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl BoundingBox {
    /// Returns bbox as Overpass API format string: "south,west,north,east"
    pub fn to_overpass_string(&self) -> String {
        format!("{},{},{},{}", self.south, self.west, self.north, self.east)
    }
}

/// Transport type for both configuration and runtime detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    Tram,
    Bus,
    Subway,
    Train,
    Ferry,
    /// Used when transport type cannot be determined from OSM data
    #[serde(other)]
    Unknown,
}

impl TransportType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportType::Tram => "tram",
            TransportType::Bus => "bus",
            TransportType::Subway => "subway",
            TransportType::Train => "train",
            TransportType::Ferry => "ferry",
            TransportType::Unknown => "unknown",
        }
    }
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;

        serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),
    #[error("Failed to parse config: {0}")]
    ParseError(String),
}
