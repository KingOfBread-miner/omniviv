/// EFA (Elektronische Fahrplanauskunft) API Service
///
/// This module provides access to the Bahnland Bayern EFA API for real-time
/// tram departure and arrival information.
///
/// # API Documentation
///
/// The EFA XML_DM_REQUEST (Departure Monitor Request) API provides real-time
/// public transport information for stops/stations.
///
/// ## Base URL
/// `https://bahnland-bayern.de/efa/XML_DM_REQUEST`
///
/// ## Key Parameters
///
/// ### Required Parameters
/// - `name_dm` - Station ID (e.g., "de:09761:101" for Königsplatz)
/// - `type_dm` - Location type, usually "stop"
/// - `outputFormat` - Response format, use "rapidJSON" for JSON
///
/// ### Common Parameters
/// - `mode=direct` - Direct query mode
/// - `depType` - Type of events: "stopEvents" (default), "arrival"
/// - `limit` - Maximum number of results (e.g., 10, 30)
/// - `useRealtime` - Include real-time data (0 or 1)
/// - `includedMeans` - Filter by transport type:
///   - 4 = Tram (Straßenbahn)
///   - 6 = Bus
///   - Other values for different transport modes
/// - `timeSpan` - Time window in minutes for departures
/// - `lineRestriction` - Filter by specific line number
/// - `itdDate` - Specific date in YYYYMMDD format
/// - `itdTime` - Specific time in HHMM format
/// - `itdDateTimeDepArr` - "dep" for departures (default), "arr" for arrivals
///
/// ### Optional Parameters
/// - `includeCompleteStopSeq=1` - Include complete stop sequence (not always available)
///
/// ## Response Structure
///
/// ### Main Fields
/// - `version` - API version
/// - `locations` - Array of location information
/// - `stopEvents` - Array of departure/arrival events
///
/// ### StopEvent Fields
/// - `location` - Platform/stop information with coordinates
/// - `departureTimePlanned` - Scheduled departure time (ISO 8601)
/// - `departureTimeEstimated` - Real-time estimated departure (if available)
/// - `departureDelay` - Delay in minutes (if available)
/// - `arrivalTimePlanned` - Scheduled arrival time
/// - `arrivalTimeEstimated` - Real-time estimated arrival (if available)
/// - `transportation` - Vehicle information:
///   - `id` - Trip ID
///   - `name` - Line name (e.g., "Straßenbahn 4")
///   - `number` - Line number (e.g., "4")
///   - `product` - Transport product details:
///     - `class` - Product class (4 = tram)
///     - `name` - Product name
///   - `destination` - Final destination of this trip
///   - `origin` - Origin of this trip
/// - `infos` - Service alerts and information (optional array):
///   - `priority` - Alert priority level
///   - `id` - Unique info ID
///   - `type` - Info type (e.g., "lineInfo")
///   - `infoLinks` - Array of info details with URLs and content
///
/// ## Transport Product Classes
/// - 4 = Straßenbahn (Tram)
/// - 6 = Bus
/// - (other classes available for different transport modes)
///
/// ## Station ID Format
/// Station IDs follow the format: `de:{area_code}:{station_number}`
/// - Example: `de:09761:101` (Augsburg Königsplatz)
/// - Example: `de:09761:422` (Oberhausen Nord P+R)
///
/// Use the STOPFINDER_REQUEST API to search for stations by name:
/// `https://bahnland-bayern.de/efa/XML_STOPFINDER_REQUEST?outputFormat=rapidJSON&type_sf=any&name_sf={search_term}`
///
/// ## Example Usage
///
/// Get next 10 tram departures from Königsplatz with real-time data:
/// ```
/// GET https://bahnland-bayern.de/efa/XML_DM_REQUEST?
///     mode=direct&
///     name_dm=de:09761:101&
///     type_dm=stop&
///     depType=stopEvents&
///     outputFormat=rapidJSON&
///     limit=10&
///     includedMeans=4&
///     useRealtime=1
/// ```
///
/// Get departures for tomorrow at 8:00 AM:
/// ```
/// GET https://bahnland-bayern.de/efa/XML_DM_REQUEST?
///     mode=direct&
///     name_dm=de:09761:101&
///     type_dm=stop&
///     depType=stopEvents&
///     outputFormat=rapidJSON&
///     limit=10&
///     includedMeans=4&
///     itdDate=20251125&
///     itdTime=0800
/// ```
///
/// ## Notes
/// - Times are returned in UTC (ISO 8601 format with Z suffix)
/// - Real-time data (`departureTimeEstimated`) may not always be available
/// - Service alerts are included in the `infos` array when available
/// - The API supports HTTPS only
/// - Response times are typically under 1 second
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, trace};

const EFA_BASE_URL: &str = "https://bahnland-bayern.de/efa/XML_DM_REQUEST";

/// Platform information with OSM data
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub coord: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub osm_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub osm_tags: Option<std::collections::HashMap<String, String>>,
}

/// Station information with platforms
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Station {
    pub station_id: String,
    pub station_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    pub coord: Option<Vec<f64>>,
    pub platforms: Vec<Platform>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaLocation {
    pub id: String,
    #[serde(rename = "isGlobalId")]
    pub is_global_id: Option<bool>,
    pub name: String,
    #[serde(rename = "disassembledName")]
    pub disassembled_name: Option<String>,
    pub coord: Option<Vec<f64>>,
    #[serde(rename = "type")]
    pub location_type: String,
    #[serde(rename = "productClasses")]
    pub product_classes: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaProduct {
    pub id: i32,
    pub class: i32,
    pub name: String,
    #[serde(rename = "iconId")]
    pub icon_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaDestination {
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub dest_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EfaTransportation {
    pub id: String,
    pub name: String,
    pub number: String,
    pub trip_code: Option<i64>,
    pub vehicle_id: Option<String>,
    pub product: EfaProduct,
    pub destination: EfaDestination,
    pub origin: Option<EfaDestination>,
    /// Catch-all for any additional fields not explicitly mapped
    pub additional_fields: serde_json::Value,
}

impl<'de> Deserialize<'de> for EfaTransportation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct EfaTransportationHelper {
            id: String,
            name: String,
            number: String,
            #[serde(rename = "tripCode")]
            trip_code: Option<i64>,
            #[serde(rename = "vehicleId")]
            vehicle_id: Option<String>,
            product: EfaProduct,
            destination: EfaDestination,
            origin: Option<EfaDestination>,
            #[serde(flatten)]
            additional_fields: serde_json::Value,
        }

        let helper = EfaTransportationHelper::deserialize(deserializer)?;

        // Extract tripCode from properties if not at top level
        let trip_code = helper.trip_code.or_else(|| {
            helper.additional_fields
                .get("properties")
                .and_then(|p| p.get("tripCode"))
                .and_then(|tc| tc.as_i64())
        });

        Ok(EfaTransportation {
            id: helper.id,
            name: helper.name,
            number: helper.number,
            trip_code,
            vehicle_id: helper.vehicle_id,
            product: helper.product,
            destination: helper.destination,
            origin: helper.origin,
            additional_fields: helper.additional_fields,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaInfoLink {
    #[serde(rename = "urlText")]
    pub url_text: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaInfo {
    pub priority: String,
    pub id: String,
    pub version: Option<i32>,
    #[serde(rename = "type")]
    pub info_type: String,
    #[serde(rename = "infoLinks")]
    pub info_links: Option<Vec<EfaInfoLink>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaOnwardLocation {
    pub id: String,
    #[serde(rename = "isGlobalId")]
    pub is_global_id: Option<bool>,
    pub name: String,
    #[serde(rename = "disassembledName")]
    pub disassembled_name: Option<String>,
    pub coord: Option<Vec<f64>>,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    #[serde(rename = "arrivalTimePlanned")]
    pub arrival_time_planned: Option<String>,
    #[serde(rename = "arrivalTimeEstimated")]
    pub arrival_time_estimated: Option<String>,
    #[serde(rename = "departureTimePlanned")]
    pub departure_time_planned: Option<String>,
    #[serde(rename = "departureTimeEstimated")]
    pub departure_time_estimated: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaStopEvent {
    pub location: EfaLocation,
    #[serde(rename = "departureTimePlanned")]
    pub departure_time_planned: Option<String>,
    #[serde(rename = "departureTimeEstimated")]
    pub departure_time_estimated: Option<String>,
    #[serde(rename = "departureDelay")]
    pub departure_delay: Option<i32>,
    #[serde(rename = "arrivalTimePlanned")]
    pub arrival_time_planned: Option<String>,
    #[serde(rename = "arrivalTimeEstimated")]
    pub arrival_time_estimated: Option<String>,
    #[serde(rename = "arrivalDelay")]
    pub arrival_delay: Option<i32>,
    pub transportation: EfaTransportation,
    pub infos: Option<Vec<EfaInfo>>,
    #[serde(rename = "onwardLocations")]
    pub onward_locations: Option<Vec<EfaOnwardLocation>>,
    #[serde(rename = "previousLocations")]
    pub previous_locations: Option<Vec<EfaOnwardLocation>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct EfaDepartureMonitorResponse {
    pub version: String,
    pub locations: Vec<EfaLocation>,
    #[serde(rename = "stopEvents")]
    pub stop_events: Vec<EfaStopEvent>,
}

/// Get stop events for a station for caching
///
/// # Arguments
/// * `station_id` - Station ID (e.g., "de:09761:101")
/// * `metrics` - Optional metrics tracker to record requests
///
/// # Returns
/// Typed departure monitor response containing stop events
pub async fn get_stop_events(
    station_id: &str,
    metrics: Option<&super::metrics::MetricsTracker>,
) -> Result<EfaDepartureMonitorResponse, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}?mode=direct&name_dm={}&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&limit=20&useRealtime=1&includedMeans=4&coordOutputFormat=EPSG:4326&includeCompleteStopSeq=1",
        EFA_BASE_URL,
        urlencoding::encode(station_id)
    );

    trace!(url = %url, station_id = %station_id, "Fetching stop events for cache");

    // Record request in metrics
    if let Some(m) = metrics {
        m.record_request().await;
    }

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error body".to_string());
        return Err(format!("HTTP {}: {}", status, error_body).into());
    }

    let data: EfaDepartureMonitorResponse = response.json().await?;

    trace!(
        station_id = %station_id,
        events = data.stop_events.len(),
        "Retrieved stop events for cache"
    );

    Ok(data)
}

/// Get station information with stop events from EFA API as raw JSON
///
/// Queries the EFA Departure Monitor API to get stop events which include
/// platform information in the location details.
///
/// # Arguments
/// * `station_id` - Station ID (e.g., "de:09761:312")
///
/// # Returns
/// Full JSON response from EFA API including locations and stopEvents
pub async fn get_station_info(
    station_id: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}?mode=direct&name_dm={}&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&includeCompleteStopSeq=1&useRealtime=1&limit=1&includedMeans=4&coordOutputFormat=EPSG:4326",
        EFA_BASE_URL,
        urlencoding::encode(station_id)
    );

    trace!(url = %url, station_id = %station_id, "Fetching station info with stop events");

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error body".to_string());
        return Err(format!("HTTP {}: {}", status, error_body).into());
    }

    let response_text = response.text().await?;
    let data: Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("JSON parse error: {}. Response body: {}", e, response_text))?;

    info!(station_id = %station_id, "Retrieved station info with stop events");

    Ok(data)
}

/// Extract the parent station ID from a full IFOPT reference
/// Example: "de:09761:692:31:a" -> "de:09761:692"
fn extract_station_id(ifopt_ref: &str) -> String {
    let parts: Vec<&str> = ifopt_ref.split(':').collect();
    if parts.len() >= 3 {
        format!("{}:{}:{}", parts[0], parts[1], parts[2])
    } else {
        ifopt_ref.to_string()
    }
}

/// Fetch platform name from EFA API using IFOPT reference
///
/// Queries the EFA API with an IFOPT reference and extracts the platform name
/// from the stop events response.
///
/// # Arguments
/// * `ifopt_ref` - Full IFOPT reference (e.g., "de:09761:401:1:1")
///
/// # Returns
/// Result with platform name or error
pub async fn fetch_platform_name(ifopt_ref: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{}?mode=direct&name_dm={}&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&includeCompleteStopSeq=1&useRealtime=1&limit=1&includedMeans=4&coordOutputFormat=EPSG:4326",
        EFA_BASE_URL,
        urlencoding::encode(ifopt_ref)
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {}: Failed to fetch platform info", status).into());
    }

    let response_text = response.text().await?;
    let data: Value = serde_json::from_str(&response_text)?;

    // Try to extract platform name from the response
    // First, try from stopEvents
    if let Some(stop_events) = data.get("stopEvents").and_then(|se| se.as_array()) {
        for event in stop_events {
            if let Some(location) = event.get("location") {
                let platform_id = location.get("id").and_then(|id| id.as_str());

                // Check if this is the platform we're looking for
                if platform_id == Some(ifopt_ref) {
                    // Try to get platform name from disassembledName or properties.platformName
                    if let Some(name) = location
                        .get("disassembledName")
                        .and_then(|n| n.as_str())
                        .or_else(|| {
                            location
                                .get("properties")
                                .and_then(|p| p.get("platformName"))
                                .and_then(|n| n.as_str())
                        })
                    {
                        return Ok(name.to_string());
                    }
                }
            }
        }
    }

    // Fallback: try from locations array
    if let Some(locations) = data.get("locations").and_then(|l| l.as_array()) {
        if let Some(location) = locations.first() {
            if let Some(name) = location
                .get("disassembledName")
                .and_then(|n| n.as_str())
                .or_else(|| location.get("name").and_then(|n| n.as_str()))
            {
                return Ok(name.to_string());
            }
        }
    }

    Err(format!("Could not extract platform name for {}", ifopt_ref).into())
}

/// Extract station and platform information from EFA response
///
/// Transforms the raw EFA JSON response into a compact format with only
/// essential station and platform information.
///
/// # Arguments
/// * `efa_response` - Raw JSON response from EFA API
///
/// # Returns
/// Result with Station data or detailed error message
pub fn extract_compact_station_data(efa_response: &Value) -> Result<Station, String> {
    // Extract station info from locations array
    let locations = efa_response
        .get("locations")
        .ok_or_else(|| "Missing 'locations' field in EFA response".to_string())?
        .as_array()
        .ok_or_else(|| "'locations' field is not an array".to_string())?;

    if locations.is_empty() {
        return Err(format!(
            "Empty locations array in EFA response. Full response: {}",
            serde_json::to_string_pretty(efa_response)
                .unwrap_or_else(|_| "Unable to serialize".to_string())
        ));
    }

    let station = &locations[0];
    let full_id = station
        .get("id")
        .ok_or_else(|| "Missing 'id' field in station location".to_string())?
        .as_str()
        .ok_or_else(|| "'id' field is not a string".to_string())?
        .to_string();

    // Extract parent station ID (first 3 parts of IFOPT)
    let station_id = extract_station_id(&full_id);

    // Get both full name and short name
    let full_name = station
        .get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    let short_name = station
        .get("disassembledName")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    // Use short_name if available, fallback to full_name for station_name
    let station_name = short_name
        .clone()
        .or_else(|| full_name.clone())
        .ok_or_else(|| {
            "Missing 'disassembledName' and 'name' field in station location".to_string()
        })?;

    let station_coord = station.get("coord").and_then(|c| {
        c.as_array().and_then(|arr| {
            if arr.len() >= 2 {
                Some(vec![arr[0].as_f64()?, arr[1].as_f64()?])
            } else {
                None
            }
        })
    });

    // Extract platforms from stopEvents
    let mut platforms = Vec::new();
    let mut seen_platform_ids = std::collections::HashSet::new();

    if let Some(stop_events) = efa_response.get("stopEvents").and_then(|se| se.as_array()) {
        for event in stop_events {
            if let Some(location) = event.get("location") {
                let platform_id = match location.get("id").and_then(|id| id.as_str()) {
                    Some(id) => id.to_string(),
                    None => continue,
                };

                // Skip if we've already seen this platform
                if !seen_platform_ids.insert(platform_id.clone()) {
                    continue;
                }

                // Try to get platform name from disassembledName or properties.platformName
                let platform_name = location
                    .get("disassembledName")
                    .and_then(|n| n.as_str())
                    .or_else(|| {
                        location
                            .get("properties")
                            .and_then(|p| p.get("platformName"))
                            .and_then(|n| n.as_str())
                    })
                    .unwrap_or("Unknown")
                    .to_string();

                let platform_coord = location.get("coord").and_then(|c| {
                    c.as_array().and_then(|arr| {
                        if arr.len() >= 2 {
                            Some(vec![arr[0].as_f64()?, arr[1].as_f64()?])
                        } else {
                            None
                        }
                    })
                });

                platforms.push(Platform {
                    id: platform_id,
                    name: platform_name,
                    coord: platform_coord,
                    osm_id: None,
                    osm_tags: None,
                });
            }
        }
    }

    info!(
        station_id = %station_id,
        platform_count = platforms.len(),
        "Extracted station data"
    );

    Ok(Station {
        station_id,
        station_name,
        full_name,
        short_name,
        coord: station_coord,
        platforms,
    })
}
