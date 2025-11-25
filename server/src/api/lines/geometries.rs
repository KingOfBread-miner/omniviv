use crate::api::AppState;
use crate::models::{LineGeometry, LineGeometryRequest};
use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use tracing::debug;

/// Get geometry for a specific line by line reference number
#[utoipa::path(
    get,
    path = "/api/lines/{line_ref}/geometries",
    params(
        ("line_ref" = String, Path, description = "Line reference number (e.g., '1', '2', '3')")
    ),
    responses(
        (status = 200, description = "Geometry for the requested tram line", body = LineGeometry),
        (status = 404, description = "Line not found")
    ),
    tag = "lines"
)]
pub async fn get_line_geometry(
    State(state): State<AppState>,
    Path(line_ref): Path<String>,
) -> Result<Response, StatusCode> {
    debug!(line_ref = %line_ref, "Fetching geometry for line");

    // Find all line variants for this ref number
    let lines_for_ref: Vec<_> = state
        .lines
        .iter()
        .filter(|l| l.ref_number.as_deref() == Some(&line_ref))
        .collect();

    if lines_for_ref.is_empty() {
        debug!(line_ref = %line_ref, "Line not found");
        return Err(StatusCode::NOT_FOUND);
    }

    let first_line = lines_for_ref.first().unwrap();
    let color = first_line
        .color
        .clone()
        .unwrap_or_else(|| "#888888".to_string());

    // Collect all unique way IDs for this line
    let all_way_ids: Vec<i64> = lines_for_ref
        .iter()
        .flat_map(|l| l.way_ids.iter().copied())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Use cached geometries
    let way_map = &*state.geometry_cache;

    // Build segments from cached geometries
    let segments: Vec<Vec<[f64; 2]>> = all_way_ids
        .iter()
        .filter_map(|id| way_map.get(id).cloned())
        .collect();

    let line_geometry = LineGeometry {
        line_ref,
        color,
        segments,
    };

    debug!("Successfully fetched line geometry");

    // Build response with cache headers
    // Cache for 24 hours since line geometries rarely change
    let mut response = Json(line_geometry).into_response();
    let headers = response.headers_mut();

    // Cache-Control: public (can be cached by any cache), max-age=86400 (24 hours)
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=86400".parse().unwrap(),
    );

    Ok(response)
}

/// Get geometries for multiple lines
#[utoipa::path(
    post,
    path = "/api/lines/geometries",
    request_body = LineGeometryRequest,
    responses(
        (status = 200, description = "Geometries for requested tram lines", body = Vec<LineGeometry>)
    ),
    tag = "lines"
)]
pub async fn get_line_geometries(
    State(state): State<AppState>,
    Json(request): Json<LineGeometryRequest>,
) -> Result<Response, StatusCode> {
    debug!(
        line_count = request.line_refs.len(),
        "Fetching line geometries"
    );

    // Collect all way IDs for the requested lines
    let mut line_ways: Vec<(String, String, Vec<i64>)> = Vec::new();
    for line_ref in &request.line_refs {
        let lines_for_ref: Vec<_> = state
            .lines
            .iter()
            .filter(|l| l.ref_number.as_deref() == Some(line_ref))
            .collect();

        if let Some(first_line) = lines_for_ref.first() {
            let color = first_line
                .color
                .clone()
                .unwrap_or_else(|| "#888888".to_string());
            let all_way_ids: Vec<i64> = lines_for_ref
                .iter()
                .flat_map(|l| l.way_ids.iter().copied())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            line_ways.push((line_ref.clone(), color, all_way_ids));
        }
    }

    // Use cached geometries
    let way_map = &*state.geometry_cache;

    // Build line geometries
    let line_geometries: Vec<LineGeometry> = line_ways
        .into_iter()
        .map(|(line_ref, color, way_ids)| {
            let segments: Vec<Vec<[f64; 2]>> = way_ids
                .iter()
                .filter_map(|id| way_map.get(id).cloned())
                .collect();

            LineGeometry {
                line_ref,
                color,
                segments,
            }
        })
        .collect();

    debug!(
        geometry_count = line_geometries.len(),
        "Successfully fetched line geometries"
    );

    // Build response with cache headers
    // Cache for 24 hours since line geometries rarely change
    let mut response = Json(line_geometries).into_response();
    let headers = response.headers_mut();

    // Cache-Control: public (can be cached by any cache), max-age=86400 (24 hours)
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=86400".parse().unwrap(),
    );

    Ok(response)
}
