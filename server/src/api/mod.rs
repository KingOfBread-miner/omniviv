pub mod areas;
pub mod departures;
pub mod error;
pub mod routes;
pub mod stations;
pub mod vehicles;

pub use error::{ErrorResponse, internal_error};

use axum::Router;
use sqlx::SqlitePool;

use crate::sync::DepartureStore;

pub fn router(pool: SqlitePool, departure_store: DepartureStore) -> Router {
    Router::new()
        .nest("/areas", areas::router(pool.clone()))
        .nest("/routes", routes::router(pool.clone()))
        .nest("/stations", stations::router(pool.clone()))
        .nest("/departures", departures::router(departure_store.clone()))
        .nest("/vehicles", vehicles::router(pool, departure_store))
}
