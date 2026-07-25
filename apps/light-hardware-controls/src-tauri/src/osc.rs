use light_control::{ControlEvent, OscArgument, encode_osc_message, parse_osc_message};
use serde::Serialize;
use std::{
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use tauri::Emitter;

struct OscClient {
    socket: Arc<UdpSocket>,
    target: SocketAddr,
    desk: String,
    id: String,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct ClientState(Mutex<Option<OscClient>>);

#[derive(Serialize, Clone)]
struct Feedback {
    address: String,
    arguments: Vec<OscArgument>,
}

impl Drop for OscClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = send(
            &self.socket,
            self.target,
            "/light/unsubscribe",
            vec![OscArgument::String(self.id.clone())],
        );
    }
}

fn send(
    socket: &UdpSocket,
    target: SocketAddr,
    address: &str,
    arguments: Vec<OscArgument>,
) -> Result<(), String> {
    let packet = encode_osc_message(address, &arguments).map_err(|error| error.to_string())?;
    socket
        .send_to(&packet, target)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn subscription_arguments(id: &str, desk: &str, port: u16) -> Vec<OscArgument> {
    vec![
        OscArgument::String(id.to_owned()),
        OscArgument::String(desk.to_owned()),
        OscArgument::Int(i32::from(port)),
        OscArgument::Int(1),
    ]
}

fn subscribe(client: &OscClient) -> Result<(), String> {
    let port = client
        .socket
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    send(
        &client.socket,
        client.target,
        "/light/subscribe",
        subscription_arguments(&client.id, &client.desk, port),
    )
}

#[tauri::command]
pub(crate) fn connect_osc(
    app: tauri::AppHandle,
    state: tauri::State<ClientState>,
    host: String,
    port: u16,
    desk: String,
) -> Result<(), String> {
    let target = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .map_err(|error| error.to_string())?;
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").map_err(|error| error.to_string())?);
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|error| error.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let client = OscClient {
        socket: Arc::clone(&socket),
        target,
        desk,
        id: uuid::Uuid::new_v4().to_string(),
        stop: Arc::clone(&stop),
    };
    subscribe(&client)?;
    spawn_feedback_listener(app, Arc::clone(&socket), Arc::clone(&stop));
    spawn_subscription_heartbeat(&client);
    *state.0.lock().map_err(|_| "OSC client lock is poisoned")? = Some(client);
    Ok(())
}

fn spawn_feedback_listener(app: tauri::AppHandle, socket: Arc<UdpSocket>, stop: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut buffer = [0u8; 65535];
        while !stop.load(Ordering::Acquire) {
            if let Ok((length, _)) = socket.recv_from(&mut buffer)
                && let Ok(ControlEvent::Osc {
                    address, arguments, ..
                }) = parse_osc_message(&buffer[..length])
            {
                let _ = app.emit("osc-feedback", Feedback { address, arguments });
            }
        }
    });
}

fn spawn_subscription_heartbeat(client: &OscClient) {
    let socket = Arc::clone(&client.socket);
    let stop = Arc::clone(&client.stop);
    let target = client.target;
    let id = client.id.clone();
    let desk = client.desk.clone();
    thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            thread::sleep(Duration::from_secs(2));
            if stop.load(Ordering::Acquire) {
                break;
            }
            if let Ok(port) = socket.local_addr().map(|address| address.port()) {
                let _ = send(
                    &socket,
                    target,
                    "/light/subscribe",
                    subscription_arguments(&id, &desk, port),
                );
            }
        }
    });
}

fn json_argument(value: serde_json::Value) -> Result<OscArgument, String> {
    match value {
        serde_json::Value::Bool(value) => Ok(OscArgument::Bool(value)),
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(OscArgument::Int(
                    i32::try_from(integer).map_err(|_| "integer is outside OSC range")?,
                ))
            } else {
                Ok(OscArgument::Float(
                    value.as_f64().ok_or("invalid OSC number")? as f32,
                ))
            }
        }
        serde_json::Value::String(value) => Ok(OscArgument::String(value)),
        _ => Err("OSC arguments must be booleans, numbers, or strings".into()),
    }
}

fn control_address(desk: &str, path: &str) -> String {
    format!("/light/{desk}/{}", path.trim_matches('/'))
}

#[tauri::command]
pub(crate) fn send_control(
    state: tauri::State<ClientState>,
    path: String,
    args: Vec<serde_json::Value>,
) -> Result<(), String> {
    let guard = state.0.lock().map_err(|_| "OSC client lock is poisoned")?;
    let client = guard.as_ref().ok_or("connect to a Light server first")?;
    send(
        &client.socket,
        client.target,
        &control_address(&client.desk, &path),
        args.into_iter()
            .map(json_argument)
            .collect::<Result<Vec<_>, _>>()?,
    )
}

#[cfg(test)]
mod tests {
    use super::{control_address, json_argument};
    use light_control::OscArgument;
    use serde_json::json;

    #[test]
    fn control_paths_remain_scoped_to_the_desk_alias() {
        assert_eq!(
            control_address("stage-left", "/page-playback/3/button/1/"),
            "/light/stage-left/page-playback/3/button/1"
        );
    }

    #[test]
    fn json_arguments_preserve_supported_osc_types() {
        assert!(matches!(
            json_argument(json!(true)),
            Ok(OscArgument::Bool(true))
        ));
        assert!(matches!(json_argument(json!(42)), Ok(OscArgument::Int(42))));
        assert!(matches!(
            json_argument(json!(0.5)),
            Ok(OscArgument::Float(value)) if value == 0.5
        ));
        assert!(matches!(
            json_argument(json!("GO")),
            Ok(OscArgument::String(value)) if value == "GO"
        ));
    }

    #[test]
    fn json_arguments_reject_unsupported_values_and_large_integers() {
        assert_eq!(
            json_argument(json!({ "nested": true })).unwrap_err(),
            "OSC arguments must be booleans, numbers, or strings"
        );
        assert_eq!(
            json_argument(json!(i64::MAX)).unwrap_err(),
            "integer is outside OSC range"
        );
    }
}
