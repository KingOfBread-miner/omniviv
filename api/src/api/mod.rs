pub mod areas;
pub mod departures;
pub mod error;
pub mod health;
pub mod issues;
pub mod routes;
pub mod stations;
pub mod vehicles;
pub mod ws;

pub use error::{ErrorResponse, internal_error};

use axum::{routing::get, Router};
use sqlx::SqlitePool;

use crate::sync::{DepartureStore, OsmIssueStore, ScheduleStore, VehicleUpdateSender};

pub fn router(
    pool: SqlitePool,
    departure_store: DepartureStore,
    schedule_store: ScheduleStore,
    time_horizon_minutes: u32,
    timezone: chrono_tz::Tz,
    issue_store: OsmIssueStore,
    vehicle_updates_tx: VehicleUpdateSender,
) -> Router {
    let ws_state = ws::WsState {
        pool: pool.clone(),
        departure_store: departure_store.clone(),
        schedule_store: schedule_store.clone(),
        time_horizon_minutes,
        timezone,
        vehicle_updates_tx,
    };

    Router::new()
        .nest("/areas", areas::router(pool.clone()))
        .nest("/routes", routes::router(pool.clone()))
        .nest("/stations", stations::router(pool.clone()))
        .nest("/departures", departures::router(departure_store.clone(), schedule_store.clone(), time_horizon_minutes, timezone))
        .nest("/vehicles", vehicles::router(pool, departure_store, schedule_store.clone(), time_horizon_minutes, timezone))
        .nest("/issues", issues::router(issue_store))
        .nest("/health", health::router(schedule_store))
        .route("/ws/vehicles", get(ws::ws_vehicles).with_state(ws_state))
}
