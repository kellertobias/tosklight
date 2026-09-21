//! Binding the administration interface, and what to do when its port is already taken.
//!
//! The administration port is the one listener an operator cannot work around from inside the
//! product: without it there is no settings page to move it from. So a run that finds the
//! conventional port occupied moves itself rather than refusing to start — but only when the port
//! is the shipped default. A port the operator typed is an instruction, and silently binding
//! somewhere else would leave a desk pointed at an address nothing answers on.
//!
//! The one case that must not move is a second ToskLight Pixel. Two Pixels on one computer fight
//! over the same Art-Net, sACN and CITP sockets, and the second one would come up half-deaf on a
//! port nobody knows. Telling them apart needs a positive answer from the occupant, not the
//! observation that something holds the socket, so the occupant is asked for its health document
//! and has to name itself as a Pixel.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use media_application::configuration::{HTTP_PORT, LOOPBACK};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How long the occupant of the port gets to identify itself. It is a loopback request to a
/// process that is already serving; a slower answer than this is not a healthy Pixel, and startup
/// must not hang on it.
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);

const HEALTH_PATH: &str = "/api/v2/health";

/// The bound administration socket and the address it actually ended up on.
#[derive(Debug)]
pub(crate) struct Administration {
    pub listener: tokio::net::TcpListener,
    /// What this run is reachable on. Every endpoint the product prints or draws comes from here,
    /// never from the configured address, so a moved port is never advertised as the old one.
    pub address: SocketAddr,
}

/// Binds the administration interface, moving off the default port when something else holds it.
pub(crate) async fn bind(listen: SocketAddr) -> anyhow::Result<Administration> {
    let error = match tokio::net::TcpListener::bind(listen).await {
        Ok(listener) => {
            let address = listener.local_addr().unwrap_or(listen);
            return Ok(Administration { listener, address });
        }
        Err(error) => error,
    };
    if error.kind() != std::io::ErrorKind::AddrInUse {
        return Err(cannot_bind(listen, &error));
    }

    let occupant = probe_address(listen);
    if pixel_is_listening(occupant).await {
        anyhow::bail!(
            "ToskLight Pixel is already running and holding the administration interface at \
             http://{occupant}/. Open it, or stop it before starting another Pixel."
        );
    }
    if !port_may_move(listen) {
        return Err(cannot_bind(listen, &error));
    }

    // Port 0 asks the operating system for a free port on the same interface, so the run keeps
    // the operator's choice of which network the interface is reachable from.
    let anywhere = SocketAddr::new(listen.ip(), 0);
    let listener = tokio::net::TcpListener::bind(anywhere)
        .await
        .map_err(|second| cannot_bind(listen, &second))?;
    let address = listener.local_addr().unwrap_or(anywhere);
    tracing::warn!(
        configured = %listen,
        bound = %address,
        "the administration port is held by another process; this run moved to a free port"
    );
    Ok(Administration { listener, address })
}

/// Only the shipped default port moves. A port the operator configured is authoritative: a desk,
/// a bookmark, or a firewall rule points at it, and a silent substitution breaks all three.
fn port_may_move(listen: SocketAddr) -> bool {
    listen.port() == HTTP_PORT
}

/// Where the occupant of a listen address is asked to identify itself.
///
/// `0.0.0.0` is every local interface rather than a destination, so the question goes to loopback,
/// which is where a Pixel on this computer answers.
fn probe_address(listen: SocketAddr) -> SocketAddr {
    if listen.ip().is_unspecified() {
        SocketAddr::new(IpAddr::V4(LOOPBACK), listen.port())
    } else {
        listen
    }
}

/// Whether the process holding the port answers as a ToskLight Pixel.
///
/// Anything else — no answer, a timeout, HTML, another product's JSON — is not a Pixel. The check
/// is deliberately one-sided: being unsure means moving to a free port, which is recoverable,
/// rather than refusing to start, which is not.
async fn pixel_is_listening(address: SocketAddr) -> bool {
    tokio::time::timeout(PROBE_TIMEOUT, ask_for_health(address))
        .await
        .ok()
        .flatten()
        .is_some_and(|response| answers_as_pixel(&response))
}

async fn ask_for_health(address: SocketAddr) -> Option<String> {
    let mut stream = tokio::net::TcpStream::connect(address).await.ok()?;
    let request =
        format!("GET {HEALTH_PATH} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.ok()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.ok()?;
    String::from_utf8(response).ok()
}

/// Parses a raw HTTP answer and reports whether it is a Pixel's health document.
fn answers_as_pixel(response: &str) -> bool {
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return false;
    };
    let answered = headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 "));
    answered && names_the_product(body)
}

/// Reads the product marker out of a health body, tolerating chunked framing around it.
fn names_the_product(body: &str) -> bool {
    let document = serde_json::from_str::<serde_json::Value>(body.trim())
        .ok()
        .or_else(|| {
            let start = body.find('{')?;
            let end = body.rfind('}')?;
            serde_json::from_str::<serde_json::Value>(body.get(start..=end)?).ok()
        });
    document
        .as_ref()
        .and_then(|document| document.get("product"))
        .and_then(serde_json::Value::as_str)
        == Some(media_http::wire::PRODUCT)
}

fn cannot_bind(listen: SocketAddr, error: &std::io::Error) -> anyhow::Error {
    let hint = if error.kind() == std::io::ErrorKind::AddrInUse {
        " Another process already holds it. Choose a different administration port in Settings, \
         or free the port."
    } else {
        " Check the listen address and operating-system network permissions."
    };
    anyhow::anyhow!("cannot bind the administration interface to {listen}: {error}.{hint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn health_body(product: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"product\":\"{product}\",\"status\":\"ok\"}}"
        )
    }

    #[test]
    fn only_the_shipped_default_port_is_allowed_to_move() {
        assert!(port_may_move("127.0.0.1:8080".parse().unwrap()));
        assert!(port_may_move("0.0.0.0:8080".parse().unwrap()));
        assert!(
            !port_may_move("127.0.0.1:9090".parse().unwrap()),
            "a port the operator typed is an instruction, not a preference"
        );
    }

    #[test]
    fn the_occupant_of_every_interface_is_asked_on_loopback() {
        assert_eq!(
            probe_address("0.0.0.0:8080".parse().unwrap()),
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            probe_address("10.42.0.8:8080".parse().unwrap()),
            "10.42.0.8:8080".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn nothing_but_a_pixel_health_document_identifies_a_pixel() {
        assert!(answers_as_pixel(&health_body("tosklight-pixel")));
        assert!(
            !answers_as_pixel(&health_body("some-other-server")),
            "another product's health document is not a Pixel"
        );
        assert!(
            !answers_as_pixel(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\"}"
            ),
            "an unnamed health document could be anything"
        );
        assert!(!answers_as_pixel(
            "HTTP/1.1 503 Service Unavailable\r\n\r\n{\"product\":\"tosklight-pixel\"}"
        ));
        assert!(!answers_as_pixel(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html>a dev server</html>"
        ));
        assert!(!answers_as_pixel("not an http answer at all"));
    }

    #[test]
    fn a_chunked_health_document_still_names_the_product() {
        assert!(names_the_product(
            "2f\r\n{\"product\":\"tosklight-pixel\",\"status\":\"ok\"}\r\n0\r\n\r\n"
        ));
    }

    #[tokio::test]
    async fn an_unoccupied_address_binds_exactly_where_it_was_asked_to() {
        let free = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = free.local_addr().unwrap();
        drop(free);

        let bound = bind(address).await.unwrap();
        assert_eq!(bound.address, address);
    }

    #[tokio::test]
    async fn an_occupied_default_port_moves_this_run_to_a_free_one() {
        let Ok(held) = tokio::net::TcpListener::bind((LOOPBACK, HTTP_PORT)).await else {
            // Something on this machine already holds 8080; the case under test cannot be staged.
            return;
        };
        let configured = held.local_addr().unwrap();

        let bound = bind(configured).await.unwrap();

        assert_ne!(bound.address.port(), HTTP_PORT);
        assert_eq!(
            bound.address.ip(),
            IpAddr::V4(LOOPBACK),
            "a moved run stays on the interface the operator configured"
        );
        assert_eq!(configured.port(), HTTP_PORT);
    }

    #[tokio::test]
    async fn an_occupied_operator_chosen_port_refuses_rather_than_moving() {
        let held = tokio::net::TcpListener::bind((LOOPBACK, 0)).await.unwrap();
        let configured = held.local_addr().unwrap();
        assert_ne!(configured.port(), HTTP_PORT);

        let refusal = bind(configured).await.unwrap_err().to_string();

        assert!(
            refusal.contains("cannot bind the administration interface"),
            "{refusal}"
        );
        assert!(refusal.contains(&configured.to_string()), "{refusal}");
    }

    #[tokio::test]
    async fn a_second_pixel_is_named_instead_of_being_moved_aside() {
        let occupied = tokio::net::TcpListener::bind((LOOPBACK, 0)).await.unwrap();
        let configured = occupied.local_addr().unwrap();
        let router = axum::Router::new().route(
            HEALTH_PATH,
            axum::routing::get(|| async {
                axum::Json(serde_json::json!({
                    "product": media_http::wire::PRODUCT,
                    "status": "ok"
                }))
            }),
        );
        tokio::spawn(async move { axum::serve(occupied, router).await });

        let refusal = bind(configured).await.unwrap_err().to_string();

        assert!(refusal.contains("already running"), "{refusal}");
        assert!(
            refusal.contains(&format!("http://{configured}/")),
            "{refusal}"
        );
    }
}
