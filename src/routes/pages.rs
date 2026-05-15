use axum::{extract::State, http::StatusCode, response::Html};
use minijinja::context;

use crate::AppState;

pub async fn resena(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let tmpl = state.tmpl.get_template("pages/resena.html").map_err(|e| {
        tracing::error!("Template resena: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl.render(context!()).map_err(|e| {
        tracing::error!("Render resena: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Html(html))
}
