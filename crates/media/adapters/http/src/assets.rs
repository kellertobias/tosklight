//! The embedded administration frontend.
//!
//! The built React application is compiled into the executable, so a Media Server is one file an
//! operator can copy to a machine and run. Nothing is read from disk at runtime and there is no
//! second thing to deploy.
//!
//! This is the router's fallback, so it is only ever reached by a request no API route claimed.

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$LIGHT_MEDIA_FRONTEND_DIR"]
struct Frontend;

const INDEX: &str = "index.html";

/// Serves one embedded file, falling back to the application shell.
///
/// A client-side route such as `/layers` is not a file; it is the shell, which then renders the
/// route. Returning 404 there would break a reload on any page but the first.
pub async fn serve(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        INDEX
    } else {
        requested
    };

    match Frontend::get(requested) {
        Some(file) => respond(requested, file.data.into_owned()),
        // A request that looks like a file and is not one is a genuine 404. Only unknown
        // *routes* fall through to the shell.
        None if requested.contains('.') => not_built_or_missing(requested),
        None => match Frontend::get(INDEX) {
            Some(index) => respond(INDEX, index.data.into_owned()),
            None => not_built_or_missing(INDEX),
        },
    }
}

fn respond(path: &str, body: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Vite fingerprints every asset it emits, so those may be cached indefinitely; the shell
    // must not be, or a browser keeps loading yesterday's application after an upgrade.
    let cache = if path == INDEX {
        "no-store"
    } else {
        "public, max-age=31536000, immutable"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref()),
            (header::CACHE_CONTROL, cache),
        ],
        body,
    )
        .into_response()
}

/// What a request gets when the frontend was never built into this executable.
///
/// A blank page teaches nobody anything, so the response says which build step is missing.
fn not_built_or_missing(path: &str) -> Response {
    if Frontend::get(INDEX).is_some() {
        return (StatusCode::NOT_FOUND, format!("no such asset: {path}")).into_response();
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CACHE_CONTROL, "no-store")],
        "The administration interface was not built into this executable. Build it with \
         `npm run build:media`, which compiles the React application before the server.",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_unknown_route_gets_the_shell_and_an_unknown_file_does_not() {
        // Both answers must be deliberate whether or not this build embedded a frontend: the
        // point of the distinction is that reloading `/layers` works while `/missing.js` 404s.
        let route = serve("/layers".parse().unwrap()).await;
        let file = serve("/missing.js".parse().unwrap()).await;

        if Frontend::get(INDEX).is_some() {
            assert_eq!(route.status(), StatusCode::OK);
            assert_eq!(file.status(), StatusCode::NOT_FOUND);
        } else {
            assert_eq!(route.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(file.status(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }
}
