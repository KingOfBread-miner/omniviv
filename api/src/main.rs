pub mod api;
mod config;
mod providers;
mod sync;

use std::sync::Arc;

use axum::{Router, routing::get};
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[cfg(feature = "dev-tools")]
use axum_sql_viewer::SqlViewerLayer;
#[cfg(feature = "dev-tools")]
use tracing_web_console::TracingLayer;

use config::Config;
use sync::SyncManager;

#[derive(OpenApi)]
#[openapi(
    info(title = "Live Tram API", version = "0.1.0"),
    paths(
        api::areas::list::list_areas,
        api::areas::list::get_area,
        api::areas::list::get_area_stats,
        api::routes::list::list_routes,
        api::routes::list::get_route,
        api::routes::list::get_route_geometry,
        api::stations::list::list_stations,
        api::departures::list_departures,
        api::departures::get_departures_by_stop,
        api::vehicles::get_vehicles_by_route,
        api::issues::list_issues,
        api::health::health_check,
    ),
    components(schemas(
        api::areas::list::Area,
        api::areas::list::AreaStats,
        api::areas::list::AreaListResponse,
        api::ErrorResponse,
        api::routes::list::Route,
        api::routes::list::RouteListResponse,
        api::routes::list::RouteDetail,
        api::routes::list::RouteStop,
        api::routes::list::RouteGeometry,
        api::stations::list::Station,
        api::stations::list::StationPlatform,
        api::stations::list::StationStopPosition,
        api::stations::list::StationListResponse,
        api::departures::DepartureListResponse,
        api::departures::StopDeparturesRequest,
        api::departures::StopDeparturesResponse,
        api::vehicles::VehiclesByRouteRequest,
        api::vehicles::VehiclesByRouteResponse,
        api::vehicles::Vehicle,
        api::vehicles::VehicleStop,
        api::issues::IssueListResponse,
        api::health::HealthResponse,
        sync::Departure,
        sync::EventType,
        sync::OsmIssue,
        sync::OsmIssueType,
    )),
    tags(
        (name = "areas", description = "Area management endpoints"),
        (name = "routes", description = "Route endpoints"),
        (name = "stations", description = "Station and platform endpoints"),
        (name = "departures", description = "Real-time departure information"),
        (name = "vehicles", description = "Live vehicle tracking"),
        (name = "issues", description = "OSM data quality issues"),
        (name = "health", description = "Service health check")
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info,sqlx=warn".into()),
        )
        .init();

    // Load config
    let config = Config::load("config.yaml").expect("Failed to load config");
    config.gtfs_sync.validate();
    tracing::info!(areas = config.areas.len(), "Loaded configuration");

    // Build CORS layer based on config
    let cors_layer = if config.cors_permissive {
        tracing::warn!("CORS: Permissive mode explicitly enabled (all origins allowed) - DO NOT USE IN PRODUCTION");
        CorsLayer::permissive()
    } else if !config.cors_origins.is_empty() {
        tracing::info!(origins = ?config.cors_origins, "CORS: Restricting to configured origins");
        let origins: Vec<_> = config
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([axum::http::header::CONTENT_TYPE])
    } else {
        panic!("CORS configuration error: Either set 'cors_origins' with allowed origins, or set 'cors_permissive: true' for development");
    };

    // Initialize SQLite database
    let cwd = std::env::current_dir().expect("Failed to get current directory");
    tracing::info!("Current working directory: {}", cwd.display());
    let db_path = cwd.join("database");
    if let Err(e) = std::fs::create_dir_all(&db_path) {
        tracing::warn!("Could not create database directory: {}", e);
    }
    let db_file = db_path.join("data.db");
    tracing::info!("Database path: {}, exists: {}", db_file.display(), db_file.exists());
    let db_url = format!("sqlite:{}?mode=rwc", db_file.display());
    let pool = SqlitePool::connect(&db_url)
        .await
        .expect("Failed to connect to SQLite database");

    // Run migrations
    let migrator = sqlx::migrate!("./migrations");
    tracing::info!(migrations = migrator.migrations.len(), "Found migrations");
    migrator
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    tracing::info!("Database migrations completed");

    // Start sync manager in background
    let sync_manager = Arc::new(
        SyncManager::new(pool.clone(), config).expect("Failed to initialize sync manager"),
    );
    let departure_store = sync_manager.departure_store();
    let schedule_store = sync_manager.schedule_store();
    let time_horizon_minutes = sync_manager.time_horizon_minutes();
    let timezone = sync_manager.timezone();
    let issue_store = sync_manager.issue_store();
    let vehicle_updates_tx = sync_manager.vehicle_updates_sender();
    let sync_manager_clone = sync_manager.clone();
    tokio::spawn(async move {
        sync_manager_clone.start().await;
    });

    // Build the app
    #[allow(unused_mut)] // mut needed when dev-tools feature is enabled
    let mut app = Router::new()
        .route("/", get(root))
        .nest("/api", api::router(pool.clone(), departure_store, schedule_store, time_horizon_minutes, timezone, issue_store, vehicle_updates_tx))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer);

    // Add dev tools only when feature is enabled
    #[cfg(feature = "dev-tools")]
    {
        let tracing_layer = TracingLayer::new("/tracing");
        app = app
            .merge(SqlViewerLayer::sqlite("/sql-viewer", pool.clone()).into_router())
            .merge(tracing_layer.into_router());
        tracing::warn!("Dev tools enabled: SQL Viewer and Tracing Console are accessible");
    }

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    tracing::info!("Server running on http://localhost:3000");
    tracing::info!("Swagger UI: http://localhost:3000/swagger-ui");
    #[cfg(feature = "dev-tools")]
    {
        tracing::info!("SQL Viewer: http://localhost:3000/sql-viewer");
        tracing::info!("Tracing Console: http://localhost:3000/tracing");
    }

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

async fn root() -> &'static str {
    "Live Tram API"
}
