use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
