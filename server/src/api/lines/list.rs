use crate::{
    api::AppState,
    models::{LineWithPlatforms, LinesListResponse, PlatformInfo},
};
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/api/lines/list",
    responses(
        (status = 200, description = "List of all tram lines with platform IDs including ref:IFOPT from OSM data", body = LinesListResponse)
    ),
    tag = "lines"
)]
pub async fn get_lines(State(state): State<AppState>) -> Response {
    // Convert OSM lines with IFOPT data to the API response format
    let lines: Vec<LineWithPlatforms> = state
        .lines_with_ifopt
        .iter()
        .filter_map(|osm_line| {
            // Only include lines that have a ref_number
            osm_line.ref_number.as_ref().map(|ref_number| {
                let platforms: Vec<PlatformInfo> = osm_line
                    .platforms
                    .iter()
                    .map(|platform| PlatformInfo {
                        // Use ref:IFOPT as the ID if available, otherwise use OSM ID
                        id: platform
                            .ref_ifopt
                            .clone()
                            .unwrap_or_else(|| format!("osm:{}", platform.osm_id)),
                        name: platform.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                        // For OSM data, use the line's "from" and "to" as station context
                        station_name: platform.name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    })
                    .collect();

                LineWithPlatforms {
                    line_number: ref_number.clone(),
                    platforms,
                }
            })
        })
        .collect();

    let total_lines = lines.len();

    let response = LinesListResponse {
        lines,
        total_lines,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(response).into_response()
}
