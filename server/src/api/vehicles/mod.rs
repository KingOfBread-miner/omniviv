mod list;

pub use list::*;

use axum::{routing::post, Router};
use sqlx::SqlitePool;

use crate::sync::DepartureStore;

#[derive(Clone)]
pub struct VehiclesState {
    pub pool: SqlitePool,
    pub departure_store: DepartureStore,
}

pub fn router(pool: SqlitePool, departure_store: DepartureStore) -> Router {
    let state = VehiclesState {
        pool,
        departure_store,
    };
    Router::new()
        .route("/by-route", post(get_vehicles_by_route))
        .with_state(state)
}
