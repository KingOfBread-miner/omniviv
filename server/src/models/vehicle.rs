use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Estimated position of a vehicle based on real-time departure data
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VehiclePosition {
    /// Unique vehicle/trip identifier
    pub vehicle_id: String,
    /// Line number (e.g., "1", "2", "3")
    pub line_number: String,
    /// Line name (e.g., "Straßenbahn 1")
    pub line_name: String,
    /// Final destination name
    pub destination: String,
    /// Estimated coordinates [longitude, latitude]
    pub coordinates: [f64; 2],
    /// Progress between from_station and to_station (0.0 to 1.0)
    pub progress: f64,
    /// Station the vehicle departed from
    pub from_station_id: String,
    /// Station the vehicle is heading to
    pub to_station_id: String,
    /// Planned departure time from from_station (ISO 8601)
    pub departure_time: String,
    /// Planned arrival time at to_station (ISO 8601)
    pub arrival_time: String,
    /// Delay in minutes (if available)
    pub delay: Option<i32>,
    /// Timestamp when this position was calculated (ISO 8601)
    pub calculated_at: String,
    /// Geometry segment between from_station and to_station for client-side interpolation
    /// Array of [longitude, latitude] coordinates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry_segment: Option<Vec<[f64; 2]>>,
}

/// Basic vehicle information extracted from stop events
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VehicleInfo {
    /// Unique vehicle/trip identifier (tripCode from EFA)
    pub vehicle_id: String,
    /// Original tripCode from EFA API
    pub trip_code: Option<i64>,
    /// Line number (e.g., "1", "2", "3")
    pub line_number: String,
    /// Line name (e.g., "Straßenbahn 1")
    pub line_name: String,
    /// Final destination name
    pub destination: String,
    /// Origin station name
    pub origin: Option<String>,
    /// Whether this vehicle is stale (not found in recent queries)
    pub is_stale: bool,
    /// Timestamp when this vehicle was last seen (ISO 8601)
    pub last_seen: String,
    /// Timestamp when this vehicle was first seen (ISO 8601)
    pub first_seen: String,
}

/// Response containing list of all unique vehicles on the network
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VehicleListResponse {
    /// Map of vehicle_id to basic vehicle info
    pub vehicles: std::collections::HashMap<String, VehicleInfo>,
    /// Total number of vehicles (active + stale)
    pub total_count: usize,
    /// Number of active vehicles (not stale)
    pub active_count: usize,
    /// Number of stale vehicles
    pub stale_count: usize,
    /// Timestamp when this list was generated
    pub timestamp: String,
}

/// Response containing all estimated vehicle positions
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VehiclePositionsResponse {
    /// Map of vehicle_id to position estimate
    pub vehicles: std::collections::HashMap<String, VehiclePosition>,
    /// Timestamp when positions were calculated
    pub timestamp: String,
}
