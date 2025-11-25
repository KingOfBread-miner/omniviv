use crate::models::{OsmTramStation, TramLine, WayGeometry};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use tracing::info;

const OVERPASS_API: &str = "https://overpass-api.de/api/interpreter";

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct RelationMember {
    #[serde(rename = "type")]
    member_type: String,
    #[serde(rename = "ref")]
    ref_id: i64,
    role: String,
}

#[derive(Debug, Deserialize)]
struct GeometryNode {
    lat: f64,
    lon: f64,
}

pub async fn load_tram_lines() -> Result<Vec<TramLine>, Box<dyn std::error::Error>> {
    info!("Loading tram lines from OpenStreetMap");

    // Load tram lines from OpenStreetMap
    let query = r#"
[out:json][timeout:60];
area["name"="Augsburg"]["boundary"="administrative"]["admin_level"="6"]->.augsburg;

// Get tram routes (relations)
rel["route"="tram"](area.augsburg);
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

    let query = r#"
[out:json][timeout:60];
area["name"="Augsburg"]["boundary"="administrative"]["admin_level"="6"]->.augsburg;

// Get all tram stops in Augsburg
(
  node["railway"="tram_stop"](area.augsburg);
  node["public_transport"="stop_position"]["tram"="yes"](area.augsburg);
  node["public_transport"="platform"]["tram"="yes"](area.augsburg);
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

/// Extract IFOPT references from OSM tram stations
///
/// Takes the first 3 colon-separated parts from ref:IFOPT tags
/// Example: "de:09761:401:1:1" -> "de:09761:401"
pub fn extract_ifopt_refs(osm_stations: &[OsmTramStation]) -> HashSet<String> {
    let mut ifopt_refs = HashSet::new();

    for station in osm_stations {
        // Check for ref:IFOPT tag
        if let Some(ifopt) = station.tags.get("ref:IFOPT") {
            // Split by colon and take first 3 parts
            let parts: Vec<&str> = ifopt.split(':').collect();
            if parts.len() >= 3 {
                let short_ref = format!("{}:{}:{}", parts[0], parts[1], parts[2]);
                ifopt_refs.insert(short_ref);
            }
        }
    }

    info!(
        ifopt_count = ifopt_refs.len(),
        "Extracted unique IFOPT references"
    );

    ifopt_refs
}
