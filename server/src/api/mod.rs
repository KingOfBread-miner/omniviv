pub mod areas;
pub mod departures;
pub mod error;
pub mod issues;
pub mod routes;
pub mod stations;
pub mod vehicles;
pub mod ws;

pub use error::{ErrorResponse, internal_error};

use axum::{routing::get, Router};
use sqlx::SqlitePool;

use crate::sync::{DepartureStore, EfaRequestSender, OsmIssueStore, VehicleUpdateSender};

pub fn router(
    pool: SqlitePool,
    departure_store: DepartureStore,
    issue_store: OsmIssueStore,
    vehicle_updates_tx: VehicleUpdateSender,
    efa_requests_tx: EfaRequestSender,
) -> Router {
    let ws_state = ws::WsState {
        pool: pool.clone(),
        departure_store: departure_store.clone(),
        vehicle_updates_tx,
    };

    let diagnostics_ws_state = ws::DiagnosticsWsState::new(efa_requests_tx);

    Router::new()
        .nest("/areas", areas::router(pool.clone()))
        .nest("/routes", routes::router(pool.clone()))
        .nest("/stations", stations::router(pool.clone()))
        .nest("/departures", departures::router(departure_store.clone()))
        .nest("/vehicles", vehicles::router(pool, departure_store))
        .nest("/issues", issues::router(issue_store))
        .route("/ws/vehicles", get(ws::ws_vehicles).with_state(ws_state))
        .route("/ws/backend-diagnostics", get(ws::ws_backend_diagnostics).with_state(diagnostics_ws_state))
}
