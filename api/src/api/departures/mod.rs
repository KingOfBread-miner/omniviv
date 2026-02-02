mod list;

pub use list::*;

use axum::{Router, routing::{get, post}};
use crate::sync::{DepartureStore, ScheduleStore};

#[derive(Clone)]
pub struct DeparturesState {
    pub departure_store: DepartureStore,
    pub schedule_store: ScheduleStore,
    pub time_horizon_minutes: u32,
    pub timezone: chrono_tz::Tz,
}

pub fn router(departure_store: DepartureStore, schedule_store: ScheduleStore, time_horizon_minutes: u32, timezone: chrono_tz::Tz) -> Router {
    let state = DeparturesState {
        departure_store,
        schedule_store,
        time_horizon_minutes,
        timezone,
    };
    Router::new()
        .route("/", get(list_departures))
        .route("/by-stop", post(get_departures_by_stop))
        .with_state(state)
}
