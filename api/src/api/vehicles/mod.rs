mod list;

pub use list::*;

use axum::{routing::post, Router};
use sqlx::SqlitePool;

use crate::sync::{DepartureStore, ScheduleStore};

#[derive(Clone)]
pub struct VehiclesState {
    pub pool: SqlitePool,
    pub departure_store: DepartureStore,
    pub schedule_store: ScheduleStore,
    pub time_horizon_minutes: u32,
    pub timezone: chrono_tz::Tz,
}

pub fn router(pool: SqlitePool, departure_store: DepartureStore, schedule_store: ScheduleStore, time_horizon_minutes: u32, timezone: chrono_tz::Tz) -> Router {
    let state = VehiclesState {
        pool,
        departure_store,
        schedule_store,
        time_horizon_minutes,
        timezone,
    };
    Router::new()
        .route("/by-route", post(get_vehicles_by_route))
        .with_state(state)
}
