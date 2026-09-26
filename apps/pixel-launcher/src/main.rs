#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use std::io::{Read, Write};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::{Child, Command, Stdio};
#[cfg(target_os = "windows")]
use std::time::{Duration, Instant};

#[cfg(any(target_os = "windows", test))]
const DEFAULT_HTTP_PORT: u16 = 8080;
#[cfg(any(target_os = "windows", test))]
const HEALTH_PATH: &str = "/api/v2/health";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = launch() {
        let _ = rfd::MessageDialog::new()
            .set_title("ToskLight Pixel")
            .set_description(format!("ToskLight Pixel could not start.\n\n{error}"))
            .set_level(rfd::MessageLevel::Error)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("ToskLight Pixel's launcher is only available on Windows.");
}

#[cfg(target_os = "windows")]
fn launch() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("the launcher has no installation directory"))?;
    let port = configured_http_port(directory)?;
    let mut launched = None;
    if !health_is_ready(port) {
        let server = directory.join("media-server.exe");
        anyhow::ensure!(
            server.is_file(),
            "the installed Media Server is missing at {}",
            server.display()
        );
        let mut command = Command::new(server);
        command
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        launched = Some(command.spawn()?);
    }
    wait_for_health(port, launched.as_mut())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn configured_http_port(directory: &std::path::Path) -> anyhow::Result<u16> {
    let configured = std::env::var_os("MEDIA_CONFIG")
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| directory.join("media/media-server.json"));
    if !configured.is_file() {
        return Ok(DEFAULT_HTTP_PORT);
    }
    let serialized = std::fs::read_to_string(&configured)?;
    let document: serde_json::Value = serde_json::from_str(&serialized).map_err(|error| {
        anyhow::anyhow!("the Media Server configuration is not usable: {error}")
    })?;
    let Some(listen) = document
        .pointer("/configuration/network/httpListen")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(DEFAULT_HTTP_PORT);
    };
    listen
        .parse::<std::net::SocketAddr>()
        .map(|address| address.port())
        .map_err(|error| anyhow::anyhow!("the Media Server HTTP address is not usable: {error}"))
}

#[cfg(target_os = "windows")]
fn wait_for_health(port: u16, mut launched: Option<&mut Child>) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if health_is_ready(port) {
            return Ok(());
        }
        if let Some(child) = launched.as_deref_mut()
            && let Some(status) = child.try_wait()?
        {
            anyhow::bail!("the Media Server exited during startup with {status}");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if let Some(child) = launched {
        let _ = child.kill();
        let _ = child.wait();
    }
    anyhow::bail!("timed out waiting for {}", administration_url(port))
}

#[cfg(target_os = "windows")]
fn health_is_ready(port: u16) -> bool {
    let health_address = format!("127.0.0.1:{port}");
    let Ok(address) = health_address.parse() else {
        return false;
    };
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&address, Duration::from_millis(250))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    if write!(
        stream,
        "GET {HEALTH_PATH} HTTP/1.1\r\nHost: {health_address}\r\nConnection: close\r\n\r\n"
    )
    .is_err()
    {
        return false;
    }
    let mut response = String::new();
    stream.read_to_string(&mut response).is_ok() && healthy_response(&response)
}

#[cfg(any(target_os = "windows", test))]
fn administration_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

/// The product marker a ToskLight Pixel's health document always carries.
///
/// The launcher decides whether to start a server from what is on the port. A healthy answer from
/// something that is not a Pixel is not a reason to skip starting one, so the answer has to name
/// the product rather than merely being well-formed.
#[cfg(any(target_os = "windows", test))]
const PRODUCT: &str = "tosklight-pixel";

#[cfg(any(target_os = "windows", test))]
fn healthy_response(response: &str) -> bool {
    let Some((headers, body)) = response.split_once("\r\n\r\n") else {
        return false;
    };
    let status_ok = headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 "));
    let Ok(document) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    let named = |field: &str| {
        document
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    status_ok
        && named("product").as_deref() == Some(PRODUCT)
        && named("status").as_deref() == Some("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_successful_pixel_health_document_is_ready() {
        assert!(healthy_response(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"product\":\"tosklight-pixel\",\"status\":\"ok\"}"
        ));
        assert!(
            !healthy_response(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\"}"
            ),
            "a health document that does not name Pixel could be any other server on the port"
        );
        assert!(!healthy_response(
            "HTTP/1.1 503 Service Unavailable\r\n\r\n{\"product\":\"tosklight-pixel\",\"status\":\"ok\"}"
        ));
        assert!(!healthy_response(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\nnot json"
        ));
    }

    #[test]
    fn launcher_targets_the_pixel_administration_contract() {
        assert_eq!(
            administration_url(DEFAULT_HTTP_PORT),
            "http://127.0.0.1:8080/"
        );
        assert_eq!(administration_url(9090), "http://127.0.0.1:9090/");
        assert_eq!(HEALTH_PATH, "/api/v2/health");
    }
}
