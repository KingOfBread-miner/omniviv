//! GTFS-based timetable provider.
//!
//! Downloads and caches a static GTFS schedule (ZIP), polls a GTFS-RT protobuf
//! feed for real-time trip updates, and produces `Departure` structs keyed by
//! IFOPT stop identifiers.

pub mod error;
pub mod realtime;
pub mod static_data;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{Duration, Utc};
use tokio::sync::RwLock;
use tracing::info;

use crate::config::GtfsSyncConfig;
use crate::sync::Departure;

use error::GtfsError;
use static_data::GtfsSchedule;

pub struct GtfsProvider {
    client: reqwest::Client,
    config: GtfsSyncConfig,
    timezone: chrono_tz::Tz,
    schedule: Arc<RwLock<Option<GtfsSchedule>>>,
}

impl GtfsProvider {
    pub fn new(config: GtfsSyncConfig) -> Result<Self, GtfsError> {
        let client = reqwest::Client::builder()
            .user_agent("omniviv/0.2 (https://github.com/firstdorsal/omniviv)")
            .build()?;
        let timezone = config.parsed_timezone();

        Ok(Self {
            client,
            config,
            timezone,
            schedule: Arc::new(RwLock::new(None)),
        })
    }

    /// Download (if needed) and load the static GTFS schedule into memory.
    pub async fn refresh_static_schedule(&self) -> Result<(), GtfsError> {
        info!("Refreshing static GTFS schedule...");

        let zip_path = static_data::download_feed(
            &self.client,
            &self.config.static_feed_url,
            &self.config.cache_dir,
        )
        .await?;

        let path = zip_path.clone();
        let schedule = tokio::task::spawn_blocking(move || static_data::load_schedule(&path))
            .await??;

        info!(
            stops = schedule.stops.len(),
            routes = schedule.routes.len(),
            trips = schedule.trips.len(),
            "Loaded static GTFS schedule into memory"
        );

        let mut guard = self.schedule.write().await;
        *guard = Some(schedule);

        Ok(())
    }

    /// Fetch GTFS-RT and produce departures for all relevant stops.
    pub async fn fetch_departures(
        &self,
        relevant_stop_ids: &HashSet<String>,
    ) -> Result<HashMap<String, Vec<Departure>>, GtfsError> {
        let schedule_guard = self.schedule.read().await;
        let schedule = schedule_guard.as_ref().ok_or(GtfsError::ScheduleNotLoaded)?;

        let feed = realtime::fetch_feed(&self.client, &self.config.realtime_feed_url).await?;

        let now = Utc::now();
        let time_horizon = Duration::minutes(self.config.time_horizon_minutes as i64);

        let departures = realtime::process_trip_updates(
            &feed,
            schedule,
            relevant_stop_ids,
            now,
            time_horizon,
            self.timezone,
        );

        Ok(departures)
    }

    /// Check if the static schedule has been loaded.
    pub async fn is_schedule_loaded(&self) -> bool {
        self.schedule.read().await.is_some()
    }

    /// Get a shared reference to the schedule for use by API handlers.
    pub fn schedule(&self) -> Arc<RwLock<Option<GtfsSchedule>>> {
        self.schedule.clone()
    }

    /// Get the configured timezone.
    pub fn timezone(&self) -> chrono_tz::Tz {
        self.timezone
    }
}
