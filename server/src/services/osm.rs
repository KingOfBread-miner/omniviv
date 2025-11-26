use crate::models::{OsmPlatformWithIfopt, OsmTramStation, TramLine, TramLineWithIfoptPlatforms, WayGeometry};
use crate::services::efa::{Platform, Station};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use tracing::info;

const OVERPASS_API: &str = "https://overpass-api.de/api/interpreter";

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Clone, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    element_type: String,
    id: i64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
    #[serde(default)]
    members: Option<Vec<RelationMember>>,
    #[serde(default)]
    geometry: Option<Vec<GeometryNode>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RelationMember {
    #[serde(rename = "type")]
    member_type: String,
    #[serde(rename = "ref")]
    ref_id: i64,
    role: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GeometryNode {
    lat: f64,
    lon: f64,
}

pub async fn load_tram_lines() -> Result<Vec<TramLine>, Box<dyn std::error::Error>> {
    info!("Loading tram lines from OpenStreetMap");

    // Use a bounding box around Augsburg area to include all tram lines
    // Augsburg center: ~48.37°N, 10.90°E
    // Expanded bbox to cover entire tram network including suburbs
    let query = r#"
[out:json][timeout:60];
// Get tram routes (relations) in the Augsburg area
rel["route"="tram"](48.25,10.75,48.50,11.05);
out body;

// Get ways that are part of tram routes for geometry
way(r);
out geom;
"#;

    let client = reqwest::Client::new();
    let response = client
        .post(OVERPASS_API)
        .body(format!("data={}", query))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send()
        .await?;

    let overpass_data: OverpassResponse = response.json().await?;
    info!(
        elements = overpass_data.elements.len(),
        "Received elements from Overpass API"
    );

    let mut lines: Vec<TramLine> = Vec::new();

    for element in &overpass_data.elements {
        if element.element_type == "relation" {
            let tags = element.tags.as_ref();
            let mut stop_ids = Vec::new();
            let mut way_ids = Vec::new();

            if let Some(members) = &element.members {
                for member in members {
                    match member.role.as_str() {
                        "stop" | "platform" => {
                            if member.member_type == "node" {
                                stop_ids.push(member.ref_id);
                            }
                        }
                        "" => {
                            if member.member_type == "way" {
                                way_ids.push(member.ref_id);
                            }
                        }
                        _ => {}
                    }
                }
            }

            let line = TramLine {
                id: element.id,
                name: tags.and_then(|t| t.get("name").cloned()),
                ref_number: tags.and_then(|t| t.get("ref").cloned()),
                color: tags.and_then(|t| t.get("colour").cloned()),
                from: tags.and_then(|t| t.get("from").cloned()),
                to: tags.and_then(|t| t.get("to").cloned()),
                stop_ids,
                way_ids,
            };
            lines.push(line);
        }
    }

    lines.sort_by(|a, b| {
        a.ref_number
            .as_ref()
            .cmp(&b.ref_number.as_ref())
            .then_with(|| a.id.cmp(&b.id))
    });

    info!(
        lines = lines.len(),
        "Successfully loaded tram lines"
    );

    Ok(lines)
}

pub async fn fetch_way_geometries(way_ids: Vec<i64>) -> Result<Vec<WayGeometry>, Box<dyn std::error::Error>> {
    let query = format!(
        "[out:json];({});out geom;",
        way_ids
            .iter()
            .map(|id| format!("way({});", id))
            .collect::<Vec<_>>()
            .join("")
    );

    let client = reqwest::Client::new();
    let response = client
        .post(OVERPASS_API)
        .body(format!("data={}", query))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send()
        .await?;

    let overpass_data: OverpassResponse = response.json().await?;

    let geometries: Vec<WayGeometry> = overpass_data
        .elements
        .into_iter()
        .filter_map(|element| {
            if element.element_type == "way" {
                element.geometry.map(|geom| WayGeometry {
                    id: element.id,
                    coordinates: geom.iter().map(|node| [node.lon, node.lat]).collect(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(geometries)
}

pub async fn fetch_tram_stations() -> Result<Vec<OsmTramStation>, Box<dyn std::error::Error>> {
    info!("Fetching tram stations from OpenStreetMap");

    // Use a bounding box around Augsburg area to include stations outside city limits
    // Augsburg center: ~48.37°N, 10.90°E
    // Expanded bbox to cover tram network including suburbs
    let query = r#"
[out:json][timeout:60];
(
  node["railway"="tram_stop"](48.25,10.75,48.50,11.05);
  node["public_transport"="stop_position"]["tram"="yes"](48.25,10.75,48.50,11.05);
  node["public_transport"="platform"]["tram"="yes"](48.25,10.75,48.50,11.05);
);
out body;
"#;

    let client = reqwest::Client::new();
    let response = client
        .post(OVERPASS_API)
        .body(format!("data={}", query))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send()
        .await?;

    let overpass_data: OverpassResponse = response.json().await?;

    let stations: Vec<OsmTramStation> = overpass_data
        .elements
        .into_iter()
        .filter_map(|element| {
            if element.element_type == "node" {
                if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
                    let tags = element.tags.unwrap_or_default();
                    let name = tags.get("name").cloned();

                    Some(OsmTramStation {
                        id: element.id,
                        name,
                        lat,
                        lon,
                        tags,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    info!(station_count = stations.len(), "Fetched OSM tram stations");

    Ok(stations)
}

/// Extract full IFOPT references from OSM tram stations
///
/// Returns all unique ref:IFOPT values from OSM stations
/// Example: "de:09761:401:1:1" is kept as-is
pub fn extract_full_ifopt_refs(osm_stations: &[OsmTramStation]) -> Vec<String> {
    let mut ifopt_refs = HashSet::new();

    for station in osm_stations {
        // Check for ref:IFOPT tag
        if let Some(ifopt) = station.tags.get("ref:IFOPT") {
            ifopt_refs.insert(ifopt.clone());
        }
    }

    let unique_refs: Vec<String> = ifopt_refs.into_iter().collect();

    info!(
        ifopt_count = unique_refs.len(),
        "Extracted unique full IFOPT references from OSM stations"
    );

    unique_refs
}

/// Fetch tram lines with platforms including ref:IFOPT tags from OpenStreetMap
///
/// This function queries OSM for tram route relations and their associated platforms/stops,
/// extracting the ref:IFOPT tag for each platform.
pub async fn fetch_tram_lines_with_ifopt_platforms() -> Result<Vec<TramLineWithIfoptPlatforms>, Box<dyn std::error::Error>> {
    info!("Fetching tram lines with IFOPT platforms from OpenStreetMap");

    // Query to get tram routes with their platforms/stops including all tags
    let query = r#"
[out:json][timeout:60];
// Get tram routes (relations) in the Augsburg area
rel["route"="tram"](48.25,10.75,48.50,11.05);
out body;

// Get all nodes that are members of these relations
node(r);
out body;
"#;

    let client = reqwest::Client::new();
    let response = client
        .post(OVERPASS_API)
        .body(format!("data={}", query))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send()
        .await?;

    let overpass_data: OverpassResponse = response.json().await?;
    info!(
        elements = overpass_data.elements.len(),
        "Received elements from Overpass API"
    );

    // Build a map of node IDs to node data
    let mut nodes_map: HashMap<i64, OverpassElement> = HashMap::new();
    for element in &overpass_data.elements {
        if element.element_type == "node" {
            nodes_map.insert(element.id, element.clone());
        }
    }

    let mut lines: Vec<TramLineWithIfoptPlatforms> = Vec::new();

    // Process relations (tram routes)
    for element in &overpass_data.elements {
        if element.element_type == "relation" {
            let tags = element.tags.as_ref();
            let mut platforms: Vec<OsmPlatformWithIfopt> = Vec::new();

            if let Some(members) = &element.members {
                for member in members {
                    // Look for platform and stop members
                    if (member.role == "platform" || member.role == "stop" || member.role == "stop_exit_only" || member.role == "stop_entry_only")
                        && member.member_type == "node"
                    {
                        // Get the node data from our map
                        if let Some(node) = nodes_map.get(&member.ref_id) {
                            if let (Some(lat), Some(lon)) = (node.lat, node.lon) {
                                let node_tags = node.tags.as_ref().cloned().unwrap_or_default();
                                let ref_ifopt = node_tags.get("ref:IFOPT").cloned();
                                let name = node_tags.get("name").cloned();

                                let platform = OsmPlatformWithIfopt {
                                    osm_id: node.id,
                                    name,
                                    ref_ifopt,
                                    lat,
                                    lon,
                                    tags: node_tags,
                                };

                                platforms.push(platform);
                            }
                        }
                    }
                }
            }

            let line = TramLineWithIfoptPlatforms {
                line_id: element.id,
                name: tags.and_then(|t| t.get("name").cloned()),
                ref_number: tags.and_then(|t| t.get("ref").cloned()),
                color: tags.and_then(|t| t.get("colour").cloned()),
                from: tags.and_then(|t| t.get("from").cloned()),
                to: tags.and_then(|t| t.get("to").cloned()),
                platforms,
            };

            lines.push(line);
        }
    }

    // Sort lines by ref_number
    lines.sort_by(|a, b| {
        a.ref_number
            .as_ref()
            .cmp(&b.ref_number.as_ref())
            .then_with(|| a.line_id.cmp(&b.line_id))
    });

    info!(
        lines = lines.len(),
        total_platforms = lines.iter().map(|l| l.platforms.len()).sum::<usize>(),
        platforms_with_ifopt = lines.iter().flat_map(|l| &l.platforms).filter(|p| p.ref_ifopt.is_some()).count(),
        "Successfully loaded tram lines with IFOPT platforms"
    );

    Ok(lines)
}

/// Convert OSM tram stations to Station structure
///
/// This function creates Station objects from OSM data, using ref:IFOPT as platform IDs
/// and grouping platforms by their station name. Station coordinates are calculated
/// as the centroid of all platforms.
pub fn convert_osm_stations_to_stations(osm_stations: &[OsmTramStation]) -> HashMap<String, Station> {
    info!("Converting OSM stations to Station structure");

    let mut stations_map: HashMap<String, Station> = HashMap::new();

    for osm_station in osm_stations {
        let station_name = osm_station.name.clone().unwrap_or_else(|| "Unnamed Station".to_string());

        // Use ref:IFOPT as the platform ID if available, otherwise use osm:ID
        let platform_id = osm_station.tags.get("ref:IFOPT")
            .cloned()
            .unwrap_or_else(|| format!("osm:{}", osm_station.id));

        // Use station name as the station_id key (or ref:IFOPT without platform suffix if available)
        let station_id = if let Some(ifopt) = osm_station.tags.get("ref:IFOPT") {
            // Try to extract station ID from IFOPT (remove the last two components which are platform/level)
            // e.g., "de:09761:401:1:1" -> "de:09761:401"
            let parts: Vec<&str> = ifopt.split(':').collect();
            if parts.len() >= 3 {
                parts[..3].join(":")
            } else {
                ifopt.clone()
            }
        } else {
            format!("osm:{}", osm_station.id)
        };

        // Create platform from OSM data
        let platform = Platform {
            id: platform_id,
            name: station_name.clone(),
            coord: Some(vec![osm_station.lat, osm_station.lon]),
            osm_id: Some(osm_station.id),
            osm_tags: Some(osm_station.tags.clone()),
        };

        // Add platform to existing station or create new station
        stations_map
            .entry(station_id.clone())
            .and_modify(|station| {
                // Check if this platform already exists
                if !station.platforms.iter().any(|p| p.id == platform.id) {
                    station.platforms.push(platform.clone());
                }
            })
            .or_insert_with(|| Station {
                station_id: station_id.clone(),
                station_name: station_name.clone(),
                full_name: Some(station_name.clone()),
                short_name: Some(station_name),
                coord: None, // Will be calculated after all platforms are added
                platforms: vec![platform],
            });
    }

    // Calculate centroid for each station based on all its platforms
    for station in stations_map.values_mut() {
        let mut lat_sum = 0.0;
        let mut lon_sum = 0.0;
        let mut count = 0;

        for platform in &station.platforms {
            if let Some(coord) = &platform.coord {
                if coord.len() >= 2 {
                    lat_sum += coord[0];
                    lon_sum += coord[1];
                    count += 1;
                }
            }
        }

        if count > 0 {
            let centroid_lat = lat_sum / count as f64;
            let centroid_lon = lon_sum / count as f64;
            station.coord = Some(vec![centroid_lat, centroid_lon]);
        }
    }

    info!(
        station_count = stations_map.len(),
        "Converted OSM stations to Station structure with calculated centroids"
    );

    stations_map
}
