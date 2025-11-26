/// System metrics tracking
///
/// Tracks API request statistics and system performance metrics

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct RequestMetrics {
    /// Total number of requests made
    pub total_requests: u64,
    /// Requests in the last second
    pub requests_last_second: u64,
    /// Requests in the last minute
    pub requests_last_minute: u64,
    /// Average requests per second over last minute
    pub avg_rps_last_minute: f64,
    /// Current requests per second (instantaneous)
    pub current_rps: f64,
    /// Timestamp of last update
    pub last_update: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct MetricsTracker {
    /// Total requests counter
    total_requests: Arc<AtomicU64>,
    /// Request timestamps for RPS calculation
    request_times: Arc<RwLock<Vec<Instant>>>,
}

impl MetricsTracker {
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(AtomicU64::new(0)),
            request_times: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Increment request counter
    pub async fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let now = Instant::now();
        let mut times = self.request_times.write().await;

        // Add current request
        times.push(now);

        // Remove requests older than 1 minute to prevent unbounded growth
        let one_minute_ago = now - Duration::from_secs(60);
        times.retain(|&time| time > one_minute_ago);
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> RequestMetrics {
        let total = self.total_requests.load(Ordering::Relaxed);
        let times = self.request_times.read().await;
        let now = Instant::now();

        // Count requests in last second
        let one_second_ago = now - Duration::from_secs(1);
        let requests_last_second = times.iter().filter(|&&time| time > one_second_ago).count() as u64;

        // Count requests in last minute
        let one_minute_ago = now - Duration::from_secs(60);
        let requests_last_minute = times.iter().filter(|&&time| time > one_minute_ago).count() as u64;

        // Calculate average RPS over last minute
        let avg_rps_last_minute = requests_last_minute as f64 / 60.0;

        // Calculate current RPS (based on last second)
        let current_rps = requests_last_second as f64;

        RequestMetrics {
            total_requests: total,
            requests_last_second,
            requests_last_minute,
            avg_rps_last_minute,
            current_rps,
            last_update: chrono::Utc::now(),
        }
    }

    /// Get total request count
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }
}

impl Default for MetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}
