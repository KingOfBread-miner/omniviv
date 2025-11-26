pub mod lines;
pub mod stations;
pub mod system;
pub mod vehicles;

use crate::models::{
    LineGeometry, LineGeometryRequest, TramLine, TramLineWithIfoptPlatforms, VehicleInfo, VehicleListResponse, VehiclePosition,
    VehiclePositionsResponse,
};
use crate::services::efa::{
    EfaDepartureMonitorResponse, EfaDestination, EfaInfo, EfaInfoLink, EfaLocation, EfaProduct,
    EfaStopEvent, EfaTransportation, Platform, Station,
};
use crate::services::metrics::MetricsTracker;
use utoipa::OpenApi;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppState {
    /// Tram lines from OpenStreetMap
    pub lines: Arc<Vec<TramLine>>,
    /// Tram lines with IFOPT platform data from OpenStreetMap
    pub lines_with_ifopt: Arc<Vec<TramLineWithIfoptPlatforms>>,
    /// Cache of way geometries (way_id -> coordinates)
    pub geometry: Arc<HashMap<i64, Vec<[f64; 2]>>>,
    /// Stations with EFA and OSM data (station_id -> station data)
    pub stations: Arc<HashMap<String, Station>>,
    /// Real-time stop events cache (station_id -> stop events)
    pub stop_events: Arc<RwLock<HashMap<String, EfaDepartureMonitorResponse>>>,
    /// Tracked vehicles (vehicle_id -> vehicle info)
    pub vehicles: Arc<RwLock<HashMap<String, VehicleInfo>>>,
    /// Metrics tracker for EFA API requests
    pub efa_metrics: Arc<MetricsTracker>,
    /// Set of station IDs that consistently fail EFA queries (should not be queried)
    pub invalid_stations: Arc<RwLock<HashSet<String>>>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stations::list::get_stations,
        stations::stop_events::get_stop_events,
        lines::list::get_lines,
        lines::geometries::get_line_geometry,
        lines::geometries::get_line_geometries,
        vehicles::list::get_vehicles_list,
        vehicles::position_estimates::get_position_estimates,
        system::info::get_system_info,
        system::debug::analyze_vehicle_ids
    ),
    components(schemas(
        Station,
        Platform,
        TramLine,
        LineGeometry,
        LineGeometryRequest,
        EfaDepartureMonitorResponse,
        EfaStopEvent,
        EfaLocation,
        EfaTransportation,
        EfaProduct,
        EfaDestination,
        EfaInfo,
        EfaInfoLink,
        VehicleInfo,
        VehicleListResponse,
        VehiclePosition,
        VehiclePositionsResponse,
        system::info::SystemInfo,
        system::info::EfaApiMetrics,
        system::debug::VehicleIdAnalysis,
        system::debug::IdCollision,
        system::debug::CollisionExample,
        system::debug::StopEventSample
    )),
    tags(
        (name = "tram", description = "Augsburg tram network API"),
        (name = "stations", description = "Tram station information"),
        (name = "lines", description = "Tram line information and geometries"),
        (name = "vehicles", description = "Real-time vehicle position estimates"),
        (name = "system", description = "System information and metrics")
    )
)]
pub struct ApiDoc;
