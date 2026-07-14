#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs::OpenOptions, net::{SocketAddr, TcpStream}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, thread, time::{Duration, Instant}};
use tauri::{Emitter, Manager};
use serde::Serialize;

fn open_hardware_controls() -> Result<(), String> {
    let executable=std::env::current_exe().map_err(|e|e.to_string())?;
    #[cfg(target_os="macos")]
    {
        let debug=executable.ancestors().find(|p|p.file_name().is_some_and(|n|n=="target")).map(|target|target.join("debug/bundle/macos/ToskLight Hardware Controls.app"));
        let installed=std::env::var_os("HOME").map(PathBuf::from).map(|home|home.join("Applications/ToskLight Hardware Controls.app"));
        let app=debug.filter(|p|p.exists()).or_else(||installed.filter(|p|p.exists())).ok_or("ToskLight Hardware Controls.app is not installed or built")?;
        Command::new("open").arg(app).spawn().map_err(|e|e.to_string())?; return Ok(());
    }
    #[cfg(not(target_os="macos"))]
    { Command::new(executable.with_file_name(if cfg!(windows){"light-hardware-controls.exe"}else{"light-hardware-controls"})).spawn().map_err(|e|e.to_string())?; Ok(()) }
}

#[derive(Serialize)] struct ConsoleDisplay { id:String, name:String }
fn monitor_id(monitor:&tauri::window::Monitor)->String { let p=monitor.position();let s=monitor.size();format!("{}|{},{}|{}x{}",monitor.name().map(String::as_str).unwrap_or("Display"),p.x,p.y,s.width,s.height) }
#[tauri::command] fn list_console_displays(app:tauri::AppHandle)->Result<Vec<ConsoleDisplay>,String>{app.available_monitors().map_err(|e|e.to_string()).map(|items|items.into_iter().map(|m|ConsoleDisplay{id:monitor_id(&m),name:m.name().cloned().unwrap_or_else(||"Display".into())}).collect())}
#[tauri::command] fn close_console_screen(app:tauri::AppHandle,screen_id:String)->Result<(),String>{if let Some(window)=app.get_webview_window(&format!("screen-{screen_id}")){window.close().map_err(|e|e.to_string())?;}Ok(())}
#[tauri::command] fn hide_console_screen(app:tauri::AppHandle,screen_id:String)->Result<(),String>{if let Some(window)=app.get_webview_window(&format!("screen-{screen_id}")){window.hide().map_err(|e|e.to_string())?;}Ok(())}
#[tauri::command] fn exit_desktop_app(app:tauri::AppHandle){let _=app.emit("app-shutting-down",());app.exit(0);}
// Cmd+Q asks for confirmation once; the second press within the armed state actually quits.
static QUIT_ARMED:AtomicBool=AtomicBool::new(false);
#[tauri::command] fn cancel_quit(){QUIT_ARMED.store(false,Ordering::Release);}
#[tauri::command] fn frontend_ready(app:tauri::AppHandle){
    if let Some(marker)=std::env::var_os("LIGHT_DESKTOP_TEST_READY_FILE"){
        let _=std::fs::write(marker,format!("{{\"ready\":true,\"server\":\"{}\"}}",server_address()));
        if std::env::var_os("LIGHT_DESKTOP_TEST_AUTO_EXIT").is_some(){thread::spawn(move||{thread::sleep(Duration::from_millis(150));app.exit(0);});}
    }
}
fn request_quit(app:&tauri::AppHandle){if QUIT_ARMED.swap(true,Ordering::AcqRel){let _=app.emit("app-shutting-down",());app.exit(0);}else{let _=app.emit("quit-requested",());}}
#[tauri::command] fn open_console_screen(app:tauri::AppHandle,screen_id:String,title:String,display_id:Option<String>,bounds:Option<serde_json::Value>,fullscreen:bool)->Result<(),String>{
    let label=format!("screen-{screen_id}");if let Some(window)=app.get_webview_window(&label){if !window.is_visible().map_err(|e|e.to_string())? { window.show().map_err(|e|e.to_string())?; } return Ok(());}
    let monitors=app.available_monitors().map_err(|e|e.to_string())?;let monitor=display_id.as_ref().and_then(|id|monitors.iter().find(|m|monitor_id(m)==*id));if display_id.is_some()&&monitor.is_none(){return Ok(());}
    let mut builder=tauri::WebviewWindowBuilder::new(&app,label,tauri::WebviewUrl::App(format!("index.html?screen={screen_id}").into())).title(title).inner_size(1200.0,800.0).resizable(true).decorations(false);
    if let Some(value)=bounds { let x=value.get("x").and_then(|v|v.as_f64());let y=value.get("y").and_then(|v|v.as_f64());let w=value.get("width").and_then(|v|v.as_f64());let h=value.get("height").and_then(|v|v.as_f64());if let (Some(x),Some(y),Some(w),Some(h))=(x,y,w,h){builder=builder.position(x,y).inner_size(w.max(640.0),h.max(480.0));} }
    else if let Some(monitor)=monitor { let p=monitor.position();builder=builder.position(f64::from(p.x)+20.0,f64::from(p.y)+20.0); }
    builder.fullscreen(fullscreen).build().map_err(|e|e.to_string())?;Ok(())
}

struct ServerProcess { child: Arc<Mutex<Option<Child>>>, stop: Arc<AtomicBool> }

impl ServerProcess {
    fn terminate(&self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock() && let Some(child) = child.as_mut() { let _ = child.kill(); let _ = child.wait(); }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn server_address() -> SocketAddr {
    std::env::var("LIGHT_DESKTOP_TEST_BIND").ok().and_then(|value|value.parse().ok()).unwrap_or_else(||SocketAddr::from(([127,0,0,1],5000)))
}

fn server_is_running(address:SocketAddr) -> bool {
    TcpStream::connect_timeout(&address, Duration::from_millis(120)).is_ok()
}

fn debug_data_dir(executable: &Path) -> Option<PathBuf> {
    executable.ancestors().find(|path| path.file_name().is_some_and(|name| name == "target")).and_then(Path::parent).map(|root| root.join("light-data"))
}

fn launch_server(app: &tauri::AppHandle) -> Result<Option<Child>, Box<dyn std::error::Error>> {
    let address=server_address();
    if server_is_running(address) { return Ok(None); }
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or("application executable has no parent directory")?;
    let binary_name = if cfg!(windows) { "light-server.exe" } else { "light-server" };
    let bundled = directory.join(binary_name);
    let server = if bundled.is_file() { bundled } else if cfg!(debug_assertions) { debug_data_dir(&executable).and_then(|data| data.parent().map(|root| root.join("target/debug").join(binary_name))).unwrap_or(bundled) } else { bundled };
    if !server.is_file() { return Err(format!("bundled Light server is missing at {}", server.display()).into()); }
    let data_dir = std::env::var_os("LIGHT_DESKTOP_TEST_DATA_DIR").map(PathBuf::from).unwrap_or(if cfg!(debug_assertions) { debug_data_dir(&executable).unwrap_or(app.path().app_data_dir()?) } else { app.path().app_data_dir()? });
    std::fs::create_dir_all(&data_dir)?;
    let log_path = data_dir.join("light-server.log");
    let stdout = OpenOptions::new().create(true).truncate(true).write(true).open(&log_path)?;
    let stderr = stdout.try_clone()?;
    let mut child = Command::new(server).arg("--data-dir").arg(&data_dir).arg("--bind").arg(address.to_string()).stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr)).spawn()?;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if server_is_running(address) { return Ok(Some(child)); }
        if let Some(status) = child.try_wait()? { return Err(format!("bundled Light server exited during startup with {status}; see {}", log_path.display()).into()); }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill(); let _ = child.wait();
    return Err(format!("timed out waiting for bundled Light server; see {}", log_path.display()).into());
}

fn main() {
    tauri::Builder::default()
        .on_menu_event(|app,event|{match event.id().as_ref(){
            "open-hardware-controls"=>{if let Err(error)=open_hardware_controls(){eprintln!("failed to open Hardware Controls: {error}");}}
            "quit"=>request_quit(app),
            _=>{}
        }})
        .invoke_handler(tauri::generate_handler![list_console_displays,open_console_screen,close_console_screen,hide_console_screen,exit_desktop_app,cancel_quit,frontend_ready])
        .setup(|app| {
            let open=tauri::menu::MenuItemBuilder::with_id("open-hardware-controls","Open Hardware Controls").build(app)?;
            let tools=tauri::menu::SubmenuBuilder::new(app,"Tools").item(&open).build()?;
            let menu=tauri::menu::Menu::default(app.handle())?;
            // Swap the predefined Quit item (last entry of the macOS app submenu) for one we can intercept to confirm.
            #[cfg(target_os="macos")]
            if let Some(tauri::menu::MenuItemKind::Submenu(app_menu))=menu.items()?.into_iter().next(){
                if let Some(native_quit)=app_menu.items()?.last(){app_menu.remove(native_quit)?;}
                app_menu.append(&tauri::menu::MenuItemBuilder::with_id("quit","Quit ToskLight").accelerator("CmdOrCtrl+Q").build(app)?)?;
            }
            menu.append(&tools)?; app.set_menu(menu)?;
            let child = launch_server(app.handle()).map_err(|error| { eprintln!("failed to start bundled Light server: {error}"); error })?;
            if let Some(window)=app.get_webview_window("main"){
                let url=format!("http://{}",server_address());
                let encoded=serde_json::to_string(&url)?;
                if std::env::var_os("LIGHT_DESKTOP_TEST_BIND").is_some(){
                    window.eval(&format!("sessionStorage.setItem('light.test-server-url',{encoded});location.reload()"))?;
                }else if cfg!(debug_assertions){
                    window.eval(&format!("if(localStorage.getItem('light.server-url')!=={encoded}){{localStorage.setItem('light.server-url',{encoded});location.reload()}}"))?;
                }
            }
            let process = ServerProcess { child: Arc::new(Mutex::new(child)), stop: Arc::new(AtomicBool::new(false)) };
            let watched_child = Arc::clone(&process.child); let stop = Arc::clone(&process.stop); let handle = app.handle().clone();
            thread::spawn(move || while !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_secs(1));
                let needs_restart = if let Ok(mut child) = watched_child.lock() { match child.as_mut() { Some(child) => child.try_wait().ok().flatten().is_some(), None => !server_is_running(server_address()) } } else { false };
                if needs_restart { match launch_server(&handle) { Ok(next) => { if let Ok(mut child) = watched_child.lock() { *child = next; } }, Err(error) => eprintln!("failed to restart bundled Light server: {error}") } }
            });
            app.manage(process);
            Ok(())
        })
        .build(tauri::generate_context!()).expect("failed to build ToskLight control UI")
        .run(|handle,event| { if matches!(event,tauri::RunEvent::ExitRequested { .. }) { let _=handle.emit("app-shutting-down",());handle.state::<ServerProcess>().terminate(); } });
}
