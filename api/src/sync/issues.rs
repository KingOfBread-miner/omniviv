//! OSM data quality issue detection and management.

use crate::config::TransportType;
use crate::providers::osm::OsmElement;
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

/// Types of OSM data quality issues
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OsmIssueType {
    MissingIfopt,
    MissingCoordinates,
    OrphanedElement,
    MissingRouteRef,
    MissingName,
    MissingStopPosition,
    MissingPlatform,
}

impl OsmIssueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OsmIssueType::MissingIfopt => "missing_ifopt",
            OsmIssueType::MissingCoordinates => "missing_coordinates",
            OsmIssueType::OrphanedElement => "orphaned_element",
            OsmIssueType::MissingRouteRef => "missing_route_ref",
            OsmIssueType::MissingName => "missing_name",
            OsmIssueType::MissingStopPosition => "missing_stop_position",
            OsmIssueType::MissingPlatform => "missing_platform",
        }
    }
}

/// An OSM data quality issue detected during sync
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OsmIssue {
    pub osm_id: i64,
    pub osm_type: String,
    pub element_type: String,
    pub issue_type: OsmIssueType,
    pub transport_type: TransportType,
    pub description: String,
    pub osm_url: String,
    pub name: Option<String>,
    /// The ref tag value (e.g., platform letter "a", "b")
    #[serde(rename = "ref")]
    pub ref_tag: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub detected_at: String,
    /// Suggested IFOPT (for missing_ifopt issues)
    pub suggested_ifopt: Option<String>,
    /// Name of the stop that was matched
    pub suggested_ifopt_name: Option<String>,
    /// Distance in meters to the suggested stop
    pub suggested_ifopt_distance: Option<u32>,
}

impl OsmIssue {
    pub fn new(
        osm_id: i64,
        osm_type: &str,
        element_type: &str,
        issue_type: OsmIssueType,
        transport_type: TransportType,
        description: String,
        name: Option<String>,
        ref_tag: Option<String>,
        lat: Option<f64>,
        lon: Option<f64>,
    ) -> Self {
        let osm_url = format!(
            "https://www.openstreetmap.org/edit?{}={}",
            osm_type, osm_id
        );
        Self {
            osm_id,
            osm_type: osm_type.to_string(),
            element_type: element_type.to_string(),
            issue_type,
            transport_type,
            description,
            osm_url,
            name,
            ref_tag,
            lat,
            lon,
            detected_at: Utc::now().to_rfc3339(),
            suggested_ifopt: None,
            suggested_ifopt_name: None,
            suggested_ifopt_distance: None,
        }
    }

    /// Set the suggested IFOPT from EFA lookup
    pub fn with_suggested_ifopt(
        mut self,
        ifopt: String,
        name: Option<String>,
        distance: Option<u32>,
    ) -> Self {
        self.suggested_ifopt = Some(ifopt);
        self.suggested_ifopt_name = name;
        self.suggested_ifopt_distance = distance;
        self
    }
}

/// In-memory store for OSM data quality issues
pub type OsmIssueStore = Arc<RwLock<Vec<OsmIssue>>>;

/// Determine transport type from OSM element tags
pub fn determine_transport_type(element: &OsmElement) -> TransportType {
    // Check railway tag
    if let Some(railway) = element.tag("railway") {
        match railway.as_str() {
            "tram_stop" | "tram" => return TransportType::Tram,
            "subway" | "subway_entrance" => return TransportType::Subway,
            "station" | "halt" | "stop" => return TransportType::Train,
            _ => {}
        }
    }

    // Check highway tag for bus stops
    if let Some(highway) = element.tag("highway") {
        if highway == "bus_stop" {
            return TransportType::Bus;
        }
    }

    // Check amenity tag for ferry terminals
    if let Some(amenity) = element.tag("amenity") {
        if amenity == "ferry_terminal" {
            return TransportType::Ferry;
        }
    }

    // Check public_transport tag
    if let Some(pt) = element.tag("public_transport") {
        if pt == "stop_position" || pt == "platform" {
            // Try to determine from tram/bus/train/subway/ferry tags
            if element.tag("tram").is_some() || element.tag("light_rail").is_some() {
                return TransportType::Tram;
            }
            if element.tag("bus").is_some() {
                return TransportType::Bus;
            }
            if element.tag("subway").is_some() {
                return TransportType::Subway;
            }
            if element.tag("train").is_some() {
                return TransportType::Train;
            }
            if element.tag("ferry").is_some() {
                return TransportType::Ferry;
            }
        }
    }

    TransportType::Unknown
}

/// Determine transport type from route type string
pub fn transport_type_from_route(route_type: &str) -> TransportType {
    match route_type {
        "tram" | "light_rail" => TransportType::Tram,
        "bus" | "trolleybus" => TransportType::Bus,
        "subway" | "metro" => TransportType::Subway,
        "train" | "railway" | "monorail" => TransportType::Train,
        "ferry" => TransportType::Ferry,
        _ => TransportType::Unknown,
    }
}
