use axum::{
    http::{Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
struct Dist;

pub async fn spa(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = file(path) {
        return file;
    }
    if path.starts_with("assets/")
        || path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains('.'))
    {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    file("index.html").unwrap_or_else(|| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "frontend is not built",
        )
            .into_response()
    })
}

fn file(path: &str) -> Option<Response> {
    let file = Dist::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Some(
        (
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data.into_owned(),
        )
            .into_response(),
    )
}
