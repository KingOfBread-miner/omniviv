use crate::api::AppState;
use crate::models::VehiclePositionsResponse;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};

#[utoipa::path(
    get,
    path = "/api/vehicles/position_estimates",
    responses(
        (status = 200, description = "Estimated real-time positions of all tram vehicles (currently empty - position tracking being rebuilt)", body = VehiclePositionsResponse)
    ),
    tag = "vehicles"
)]
pub async fn get_position_estimates(State(_state): State<AppState>) -> Response {
    // Position tracking has been removed - return empty response
    // This will be reimplemented from scratch
    let empty_response = VehiclePositionsResponse {
        vehicles: std::collections::HashMap::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(empty_response).into_response()
}
