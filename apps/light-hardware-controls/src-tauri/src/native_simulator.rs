use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpStream, ToSocketAddrs},
    sync::Mutex,
    time::Duration,
};

#[derive(Default)]
pub(crate) struct SimulatorState(Mutex<Option<TcpStream>>);

#[tauri::command]
pub(crate) fn connect_native_simulator(
    state: tauri::State<SimulatorState>,
    host: String,
    port: u16,
) -> Result<(), String> {
    disconnect_native_simulator(state.clone())?;
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or("native simulator relay address did not resolve")?;
    if !address.ip().is_loopback() {
        return Err("native simulator relay must use a loopback address".into());
    }
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
    let mut greeting = String::new();
    reader
        .read_line(&mut greeting)
        .map_err(|error| format!("native simulator relay did not answer: {error}"))?;
    let greeting: Value = serde_json::from_str(&greeting)
        .map_err(|error| format!("native simulator relay returned invalid JSON: {error}"))?;
    if greeting.get("type").and_then(Value::as_str) != Some("ready") {
        return Err("native simulator relay did not identify itself".into());
    }
    reader
        .get_ref()
        .set_read_timeout(None)
        .map_err(|error| error.to_string())?;
    *state
        .0
        .lock()
        .map_err(|_| "simulator relay lock is poisoned")? = Some(stream);
    // The native child publishes authoritative snapshots, deltas, and validation errors.
    // Keep draining them even before the simulator UI renders those values; otherwise a busy
    // control session can fill the socket buffer and stall the supervised extension.
    std::thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn send_native_simulator_control(
    state: tauri::State<SimulatorState>,
    control: Value,
) -> Result<(), String> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "simulator relay lock is poisoned")?;
    let stream = guard
        .as_mut()
        .ok_or("connect to the native simulator extension first")?;
    let message = serde_json::json!({ "type": "control_input", "control": control });
    serde_json::to_writer(&mut *stream, &message).map_err(|error| error.to_string())?;
    stream.write_all(b"\n").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn disconnect_native_simulator(
    state: tauri::State<SimulatorState>,
) -> Result<(), String> {
    if let Some(stream) = state
        .0
        .lock()
        .map_err(|_| "simulator relay lock is poisoned")?
        .take()
    {
        let _ = stream.shutdown(Shutdown::Both);
    }
    Ok(())
}
