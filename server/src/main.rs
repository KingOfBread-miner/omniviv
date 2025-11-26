mod api;
mod models;
mod services;

use axum::http::{Method, header};
use std::{collections::HashMap, sync::Arc};
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
use services::{efa, osm};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=debug,axum::rejection=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load tram data at startup
    info!("Starting Augsburg Tram API server");
    let lines = osm::load_tram_lines().await?;

    // Try to load tram lines with IFOPT from file, otherwise fetch from OSM
    let lines_with_ifopt: Vec<crate::models::TramLineWithIfoptPlatforms> =
        if std::path::Path::new("data/tram_lines_with_ifopt.json").exists() {
            info!("Loading tram lines with IFOPT from data/tram_lines_with_ifopt.json");
            let lines_json = std::fs::read_to_string("data/tram_lines_with_ifopt.json")?;
            let lines: Vec<crate::models::TramLineWithIfoptPlatforms> =
                serde_json::from_str(&lines_json)?;
            info!(
                lines_count = lines.len(),
                "Successfully loaded tram lines with IFOPT from file"
            );
            lines
        } else {
            // Fetch and save tram lines with IFOPT platforms
            info!("Fetching tram lines with IFOPT platforms from OSM");
            let lines = osm::fetch_tram_lines_with_ifopt_platforms().await?;
            lines
        };

    // Try to load geometry cache from file, otherwise fetch from OSM
    let geometry_cache: std::collections::HashMap<i64, Vec<[f64; 2]>> =
        if std::path::Path::new("data/geometry_cache.json").exists() {
            info!("Loading geometry cache from data/geometry_cache.json");
            let cache_json = std::fs::read_to_string("data/geometry_cache.json")?;
            let cache: std::collections::HashMap<i64, Vec<[f64; 2]>> =
                serde_json::from_str(&cache_json)?;
            info!(
                cached_geometries = cache.len(),
                "Successfully loaded geometry cache from file"
            );
            cache
        } else {
            // Pre-fetch all way geometries at startup
            info!("Pre-fetching all way geometries for caching");
            let all_way_ids: Vec<i64> = lines
                .iter()
                .flat_map(|line| line.way_ids.iter().copied())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            info!(
                way_count = all_way_ids.len(),
                "Fetching geometries for ways"
            );
            let way_geometries = osm::fetch_way_geometries(all_way_ids).await?;
            let cache: std::collections::HashMap<i64, Vec<[f64; 2]>> = way_geometries
                .into_iter()
                .map(|wg| (wg.id, wg.coordinates))
                .collect();

            info!(
                cached_geometries = cache.len(),
                "Successfully cached way geometries"
            );
            cache
        };

    // Try to load station data from file, otherwise fetch from OSM
    let stations: std::collections::HashMap<String, services::efa::Station> =
        if std::path::Path::new("data/stations.json").exists() {
            info!("Loading station data from data/stations.json");
            let stations_json = std::fs::read_to_string("data/stations.json")?;
            let stations: std::collections::HashMap<String, services::efa::Station> =
                serde_json::from_str(&stations_json)?;
            info!(
                station_count = stations.len(),
                "Successfully loaded station data from file"
            );
            stations
        } else {
            // Fetch all OSM tram stations (platforms) at startup
            info!("Fetching OSM tram platforms with ref:IFOPT tags");
            let osm_platforms = osm::fetch_tram_stations().await?;
            info!(
                platform_count = osm_platforms.len(),
                platforms_with_ifopt = osm_platforms.iter().filter(|p| p.tags.contains_key("ref:IFOPT")).count(),
                "Successfully fetched OSM tram platforms"
            );

            // Convert OSM platforms to Station structure (grouped by station)
            info!("Converting OSM platforms to Station structure");
            let mut stations_map = osm::convert_osm_stations_to_stations(&osm_platforms);

            info!(
                total_stations = stations_map.len(),
                total_platforms = stations_map.values().map(|s| s.platforms.len()).sum::<usize>(),
                "Successfully created stations from OSM data"
            );

            // Augment platform names with EFA data
            info!("Fetching platform names from EFA API");
            let mut all_ifopt_refs = Vec::new();

            // Collect all IFOPT references from platforms
            for station in stations_map.values() {
                for platform in &station.platforms {
                    // Only fetch for platforms that have IFOPT refs (not osm: prefixed)
                    if !platform.id.starts_with("osm:") {
                        all_ifopt_refs.push(platform.id.clone());
                    }
                }
            }

            info!(
                ifopt_count = all_ifopt_refs.len(),
                "Collected IFOPT references to fetch platform names"
            );

            // Fetch platform names in batches
            const BATCH_SIZE: usize = 10;
            let mut platform_names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            let mut failed_count = 0;

            for (batch_idx, chunk) in all_ifopt_refs.chunks(BATCH_SIZE).enumerate() {
                let batch_start = batch_idx * BATCH_SIZE + 1;
                let batch_end = (batch_start + chunk.len() - 1).min(all_ifopt_refs.len());

                info!(
                    batch = format!("{}-{}/{}", batch_start, batch_end, all_ifopt_refs.len()),
                    "Fetching platform names batch"
                );

                // Spawn async tasks for each IFOPT in this batch
                let mut tasks = Vec::new();

                for ifopt_ref in chunk {
                    let ifopt_ref_clone = ifopt_ref.clone();
                    let task = tokio::spawn(async move {
                        match efa::fetch_platform_name(&ifopt_ref_clone).await {
                            Ok(name) => Ok((ifopt_ref_clone, name)),
                            Err(e) => {
                                tracing::debug!(
                                    ifopt_ref = %ifopt_ref_clone,
                                    error = %e,
                                    "Failed to fetch platform name"
                                );
                                Err(ifopt_ref_clone)
                            }
                        }
                    });
                    tasks.push(task);
                }

                // Wait for all tasks in this batch to complete
                let results = futures::future::join_all(tasks).await;

                // Collect successful results
                for result in results {
                    match result {
                        Ok(Ok((ifopt_ref, name))) => {
                            platform_names.insert(ifopt_ref, name);
                        }
                        Ok(Err(_)) | Err(_) => {
                            failed_count += 1;
                        }
                    }
                }

                // Small delay between batches to avoid overwhelming the API
                if batch_idx < (all_ifopt_refs.len() / BATCH_SIZE) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            }

            info!(
                fetched = platform_names.len(),
                failed = failed_count,
                total = all_ifopt_refs.len(),
                "Completed fetching platform names from EFA"
            );

            // Update platform names in stations_map with EFA data
            for station in stations_map.values_mut() {
                for platform in &mut station.platforms {
                    if let Some(efa_name) = platform_names.get(&platform.id) {
                        platform.name = efa_name.clone();
                    }
                }
            }

            info!("Updated platform names with EFA data");

            stations_map
        };

    // Save cached data to files if they don't exist
    std::fs::create_dir_all("data")?;

    if !std::path::Path::new("data/tram_lines_with_ifopt.json").exists() {
        info!("Saving tram lines with IFOPT to data/tram_lines_with_ifopt.json");
        let lines_json = serde_json::to_string_pretty(&lines_with_ifopt)?;
        std::fs::write("data/tram_lines_with_ifopt.json", lines_json)?;
        info!("Saved tram lines with IFOPT to data/tram_lines_with_ifopt.json");
    }

    if !std::path::Path::new("data/geometry_cache.json").exists() {
        info!("Saving geometry cache to data/geometry_cache.json");
        let geometry_json = serde_json::to_string_pretty(&geometry_cache)?;
        std::fs::write("data/geometry_cache.json", geometry_json)?;
        info!("Saved geometry cache to data/geometry_cache.json");
    }

    if !std::path::Path::new("data/stations.json").exists() {
        info!("Saving station data to data/stations.json");
        let stations_json = serde_json::to_string_pretty(&stations)?;
        std::fs::write("data/stations.json", stations_json)?;
        info!("Saved station data to data/stations.json");
    }

    let efa_metrics = Arc::new(services::metrics::MetricsTracker::new());

    let state = AppState {
        lines: Arc::new(lines),
        lines_with_ifopt: Arc::new(lines_with_ifopt),
        geometry: Arc::new(geometry_cache),
        stations: Arc::new(stations),
        stop_events: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        vehicles: Arc::new(std::sync::RwLock::new(HashMap::new())),
        efa_metrics: efa_metrics.clone(),
        invalid_stations: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
    };

    // Spawn background task to update stop events cache every 5 seconds
    let cache_stations = state.stations.clone();
    let cache = state.stop_events.clone();
    let vehicle_list_cache = state.vehicles.clone();
    let metrics_for_cache = efa_metrics.clone();
    let invalid_stations_for_cache = state.invalid_stations.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        // Track consecutive failures per station
        let mut failure_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        loop {
            interval.tick().await;

            // Get current set of invalid stations
            let invalid_stations_set = {
                let invalid_read = invalid_stations_for_cache.read().unwrap();
                invalid_read.clone()
            };

            // Count stations with IFOPT vs without IFOPT vs invalid stations
            let stations_with_ifopt = cache_stations
                .iter()
                .filter(|(id, _)| !id.starts_with("osm:"))
                .count();
            let stations_without_ifopt = cache_stations.len() - stations_with_ifopt;
            let invalid_count = invalid_stations_set.len();

            info!(
                stations_with_ifopt = stations_with_ifopt,
                stations_without_ifopt = stations_without_ifopt,
                invalid_stations = invalid_count,
                total = cache_stations.len(),
                "Updating stop events cache"
            );

            let mut update_tasks = Vec::new();

            // Create async tasks for each station (skip stations without IFOPT and invalid stations)
            for (station_id, _) in cache_stations.iter() {
                // Skip stations without IFOPT refs as they can't be queried in EFA API
                if station_id.starts_with("osm:") {
                    continue;
                }

                // Skip stations that are marked as invalid
                if invalid_stations_set.contains(station_id) {
                    continue;
                }

                let station_id_clone = station_id.clone();

                let metrics_clone = metrics_for_cache.clone();
                let task = tokio::spawn(async move {
                    match efa::get_stop_events(&station_id_clone, Some(&metrics_clone)).await {
                        Ok(stop_events) => Ok((station_id_clone, stop_events)),
                        Err(e) => {
                            tracing::debug!(
                                station_id = %station_id_clone,
                                error = %e,
                                "Failed to fetch stop events for station"
                            );
                            Err(station_id_clone)
                        }
                    }
                });

                update_tasks.push(task);
            }

            // Wait for all tasks to complete
            let results = futures::future::join_all(update_tasks).await;

            // Build new stop events map from results
            let mut successful_updates = 0;
            let mut failed_updates = 0;
            let mut newly_invalid_stations = Vec::new();
            let mut new_stop_events = std::collections::HashMap::new();

            for result in results {
                match result {
                    Ok(Ok((station_id, stop_events))) => {
                        new_stop_events.insert(station_id.clone(), stop_events);
                        successful_updates += 1;
                        // Reset failure count on success
                        failure_counts.remove(&station_id);
                    }
                    Ok(Err(station_id)) => {
                        failed_updates += 1;
                        // Increment failure count
                        let count = failure_counts.entry(station_id.clone()).or_insert(0);
                        *count += 1;

                        // Mark as invalid after 3 consecutive failures
                        if *count >= 3 {
                            newly_invalid_stations.push(station_id);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Task panicked while fetching stop events");
                        failed_updates += 1;
                    }
                }
            }

            // Add newly invalid stations to the invalid set
            if !newly_invalid_stations.is_empty() {
                let mut invalid_write = invalid_stations_for_cache.write().unwrap();
                for station_id in &newly_invalid_stations {
                    invalid_write.insert(station_id.clone());
                    failure_counts.remove(station_id);
                    info!(station_id = %station_id, "Marked station as invalid after 3 consecutive failures");
                }
            }

            info!(
                successful = successful_updates,
                failed = failed_updates,
                total_queried = successful_updates + failed_updates,
                "Stop events cache update completed"
            );

            // Update vehicle list from newly fetched stop events (before writing to cache)
            let previous_vehicle_list = {
                let cache_read = vehicle_list_cache.read().unwrap();
                Some(cache_read.clone())
            };

            let vehicle_list = services::vehicle_list::extract_unique_vehicles(
                &new_stop_events,
                previous_vehicle_list.as_ref(),
                300, // 5 minutes stale timeout
            );

            // Write both caches atomically
            {
                let mut cache_write = cache.write().unwrap();
                *cache_write = new_stop_events;
            }

            {
                let mut vehicle_list_cache_write = vehicle_list_cache.write().unwrap();
                *vehicle_list_cache_write = vehicle_list.vehicles;
            }
        }
    });

    info!("Background task for stop events cache and vehicle list started");

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    // Build router
    let (app, _api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(api::stations::list::get_stations))
        .routes(routes!(api::stations::stop_events::get_stop_events))
        .routes(routes!(api::lines::list::get_lines))
        .routes(routes!(api::lines::geometries::get_line_geometry))
        .routes(routes!(api::lines::geometries::get_line_geometries))
        .routes(routes!(api::vehicles::list::get_vehicles_list))
        .routes(routes!(
            api::vehicles::position_estimates::get_position_estimates
        ))
        .routes(routes!(api::system::info::get_system_info))
        .routes(routes!(api::system::debug::analyze_vehicle_ids))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .split_for_parts();

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    axum::serve(listener, app).await?;

    Ok(())
}
