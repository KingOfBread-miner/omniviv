pub mod vehicle;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use vehicle::{VehicleInfo, VehicleListResponse, VehiclePosition, VehiclePositionsResponse};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TramLine {
    pub id: i64,
    pub name: Option<String>,
    pub ref_number: Option<String>,
    pub color: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub stop_ids: Vec<i64>,
    pub way_ids: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LineGeometry {
    pub line_ref: String,
    pub color: String,
    pub segments: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LineGeometryRequest {
    pub line_refs: Vec<String>,
}

#[derive(Debug)]
pub struct WayGeometry {
    pub id: i64,
    pub coordinates: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OsmTramStation {
    pub id: i64,
    pub name: Option<String>,
    pub lat: f64,
    pub lon: f64,
    /// OSM tags like ref, operator, network, etc.
    pub tags: std::collections::HashMap<String, String>,
}

/// Platform information for a line
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlatformInfo {
    /// Platform ID (e.g., "de:09761:227:0:e")
    pub id: String,
    /// Platform name (e.g., "Königsplatz")
    pub name: String,
    /// Station name this platform belongs to
    pub station_name: String,
}

/// Line with platform IDs extracted from EFA data
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LineWithPlatforms {
    /// Line number (e.g., "1", "2", "3")
    pub line_number: String,
    /// List of all platforms served by this line
    pub platforms: Vec<PlatformInfo>,
}

/// Response containing all lines with their platform IDs
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LinesListResponse {
    /// List of lines with their platform IDs
    pub lines: Vec<LineWithPlatforms>,
    /// Total number of lines
    pub total_lines: usize,
    /// Timestamp when this list was generated
    pub timestamp: String,
}

/// Platform data from OSM with IFOPT reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmPlatformWithIfopt {
    /// OSM node ID
    pub osm_id: i64,
    /// Platform name from OSM
    pub name: Option<String>,
    /// ref:IFOPT tag value
    pub ref_ifopt: Option<String>,
    /// Latitude
    pub lat: f64,
    /// Longitude
    pub lon: f64,
    /// All OSM tags
    pub tags: std::collections::HashMap<String, String>,
}

/// Tram line with platforms including IFOPT references
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TramLineWithIfoptPlatforms {
    /// OSM relation ID
    pub line_id: i64,
    /// Line name (e.g., "Straßenbahn 1: Lechhausen Nord => Göggingen")
    pub name: Option<String>,
    /// Line reference number (e.g., "1")
    pub ref_number: Option<String>,
    /// Line color
    pub color: Option<String>,
    /// Starting point
    pub from: Option<String>,
    /// End point
    pub to: Option<String>,
    /// List of platforms with IFOPT references
    pub platforms: Vec<OsmPlatformWithIfopt>,
}
