mod api;
mod models;
mod services;

use axum::http::{Method, header};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use api::{ApiDoc, AppState};
use services::osm;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,tower_http=debug,axum::rejection=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load tram data at startup
    info!("Starting Augsburg Tram API server");
    let lines = osm::load_tram_lines().await?;

    // Pre-fetch all way geometries at startup
    info!("Pre-fetching all way geometries for caching");
    let all_way_ids: Vec<i64> = lines
        .iter()
        .flat_map(|line| line.way_ids.iter().copied())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    info!(way_count = all_way_ids.len(), "Fetching geometries for ways");
    let way_geometries = osm::fetch_way_geometries(all_way_ids).await?;
    let geometry_cache: std::collections::HashMap<i64, Vec<[f64; 2]>> = way_geometries
        .into_iter()
        .map(|wg| (wg.id, wg.coordinates))
        .collect();

    info!(
        cached_geometries = geometry_cache.len(),
        "Successfully cached way geometries"
    );

    // Fetch all OSM tram stations at startup
    info!("Fetching OSM tram stations for caching");
    let stations = osm::fetch_tram_stations().await?;
    info!(
        station_count = stations.len(),
        "Successfully cached OSM tram stations"
    );

    // Save cached data to files
    info!("Saving cached data to files");
    std::fs::create_dir_all("data")?;

    // Save geometry cache
    let geometry_json = serde_json::to_string_pretty(&geometry_cache)?;
    std::fs::write("data/geometry_cache.json", geometry_json)?;
    info!("Saved geometry cache to data/geometry_cache.json");

    // Save OSM stations
    let stations_json = serde_json::to_string_pretty(&stations)?;
    std::fs::write("data/osm_stations.json", stations_json)?;
    info!("Saved OSM stations to data/osm_stations.json");

    let state = AppState {
        lines: Arc::new(lines),
        geometry_cache: Arc::new(geometry_cache),
        stations: Arc::new(stations),
    };

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    // Build router
    let (app, _api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(api::stations::list::get_stations))
        .routes(routes!(api::lines::list::get_lines))
        .routes(routes!(api::lines::geometries::get_line_geometry))
        .routes(routes!(api::lines::geometries::get_line_geometries))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .split_for_parts();

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    axum::serve(listener, app).await?;

    Ok(())
}
