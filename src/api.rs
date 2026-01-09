use crate::db::Database;
use crate::modem::ModemManager;
use crate::utils::parse_rfc3339_timestamp;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct MessageQuery {
    after: Option<String>,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = if self.success {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (status, Json(self)).into_response()
    }
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(error),
        }
    }

    pub fn error_with_status(error: String, status: StatusCode) -> (StatusCode, Json<ApiResponse<()>>) {
        (status, Json(ApiResponse::<()>::error(error)))
    }
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<Database>>,
    modem_manager: Arc<ModemManager>,
}

pub fn create_router(db: Arc<Mutex<Database>>, modem_manager: Arc<ModemManager>) -> Router {
    let state = AppState {
        db,
        modem_manager,
    };

    Router::new()
        .route("/messages/:imei", get(get_messages))
        .with_state(state)
}

pub fn create_metrics_router(modem_manager: Arc<ModemManager>) -> Router {
    let state = AppState {
        db: Arc::new(Mutex::new(Database::new(":memory:").unwrap())),
        modem_manager,
    };

    Router::new()
        .route("/modems", get(get_modems))
        .route("/metrics", get(get_metrics))
        .route("/health", get(health_check))
        .with_state(state)
}

async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("OK".to_string()))
}

async fn get_modems(State(state): State<AppState>) -> Response {
    let modems = state.modem_manager.get_modems().await;

    match modems {
        Ok(modems) => Json(ApiResponse::success(modems)).into_response(),
        Err(e) => ApiResponse::<()>::error_with_status(
            format!("Failed to get modems: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response(),
    }
}

async fn get_metrics(State(state): State<AppState>) -> Response {
    let modem_count = match state.modem_manager.get_modems().await {
        Ok(modems) => modems.len(),
        Err(_) => 0,
    };

    let response = format!(
        "# HELP modem_count Total number of modems\n\
         # TYPE modem_count gauge\n\
         modem_count {}\n",
        modem_count
    );

    response.into_response()
}

async fn get_messages(
    State(state): State<AppState>,
    Path(imei): Path<String>,
    Query(params): Query<MessageQuery>,
) -> Response {
    // Parse and validate 'after' timestamp parameter if provided
    let after = if let Some(after_str) = params.after {
        match parse_rfc3339_timestamp(&after_str) {
            Ok(dt) => Some(dt),
            Err(e) => {
                return ApiResponse::<()>::error_with_status(
                    format!("Invalid 'after' timestamp format. Expected RFC3339: {}", e),
                    StatusCode::BAD_REQUEST,
                )
                .into_response();
            }
        }
    } else {
        None
    };

    // Query database
    let messages = {
        let db = state.db.lock().await;
        db.get_messages(Some(&imei), after)
    };

    match messages {
        Ok(messages) => Json(ApiResponse::success(messages)).into_response(),
        Err(e) => ApiResponse::<()>::error_with_status(
            format!("Database error: {}", e),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .into_response(),
    }
}
