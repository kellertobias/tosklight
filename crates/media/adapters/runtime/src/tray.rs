//! The Media Server's presence on the desktop: a menu bar item on macOS, a notification-area item
//! on Windows.
//!
//! A media server has no main window. Its outputs are full-screen surfaces on the monitors an
//! operator assigned, and with none assigned it draws nothing at all — so without this the process
//! is invisible, and the only way to stop it is Activity Monitor.
//!
//! The menu keeps only process-level actions plus a shortcut to the administration interface.

use crate::shutdown::{Shutdown, ShutdownReason};
use muda::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The application icon, compiled in rather than read from the bundle: the same executable runs
/// bundled and bare, and an icon that only appears in one of them is a difference nobody wants to
/// discover during a show.
const ICON: &[u8] = include_bytes!("../../../../../assets/branding/ToskLight Pixel.png");

/// What the menu bar draws. Big enough for a Retina menu bar, small enough that decoding it costs
/// nothing at startup.
const ICON_EDGE: u32 = 44;

#[cfg(target_os = "macos")]
const OPEN_FOLDER_LABEL: &str = "open Folder in Finder";

#[cfg(target_os = "windows")]
const OPEN_PIXEL_LABEL: &str = "Open ToskLight Pixel";

#[cfg(any(target_os = "macos", target_os = "windows"))]
const OPEN_SETTINGS_LABEL: &str = "Open Settings in Browser";

#[cfg(any(target_os = "macos", target_os = "windows"))]
const CONVERT_MULTIPLE_LABEL: &str = "Convert multiple files";

/// The desktop presence, held for as long as the server runs.
///
/// Dropping this removes the icon, so the caller keeps it alive; that is the whole reason it is a
/// value rather than a function that returns nothing.
pub struct Tray {
    _icon: TrayIcon,
}

/// Show the icon and wire its Quit item to `shutdown`.
///
/// Must be called on the thread that owns the platform event loop, once that loop is running:
/// macOS refuses a status item before the application has finished launching.
///
/// A failure here is not fatal. A server that cannot draw a menu bar item is still a server, and
/// taking the whole process down over its icon would turn a cosmetic problem into an outage.
pub fn show(
    shutdown: &Shutdown,
    data_directory: Option<&std::path::Path>,
    #[cfg(any(target_os = "macos", target_os = "windows"))] administration_endpoint: &str,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    bulk_import: crate::bulk_import::BulkImport,
) -> Option<Tray> {
    let icon = match icon() {
        Ok(icon) => icon,
        Err(error) => {
            tracing::warn!(%error, "the menu bar icon could not be decoded; running without one");
            return None;
        }
    };

    let menu = Menu::new();
    #[cfg(target_os = "macos")]
    let open_folder = MenuItem::new(OPEN_FOLDER_LABEL, data_directory.is_some(), None);
    #[cfg(target_os = "macos")]
    let open_folder_id = open_folder.id().clone();
    #[cfg(target_os = "macos")]
    if let Err(error) = menu.append(&open_folder) {
        tracing::warn!(%error, "the menu bar menu could not be built; running without one");
        return None;
    }
    #[cfg(target_os = "windows")]
    let open_pixel = MenuItem::new(OPEN_PIXEL_LABEL, true, None);
    #[cfg(target_os = "windows")]
    let open_pixel_id = open_pixel.id().clone();
    #[cfg(target_os = "windows")]
    if let Err(error) = menu.append(&open_pixel) {
        tracing::warn!(%error, "the notification-area menu could not be built; running without one");
        return None;
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let open_settings = MenuItem::new(OPEN_SETTINGS_LABEL, true, None);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let open_settings_id = open_settings.id().clone();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Err(error) = menu.append(&open_settings) {
        tracing::warn!(%error, "the menu bar menu could not be built; running without one");
        return None;
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let convert_multiple = MenuItem::new(CONVERT_MULTIPLE_LABEL, true, None);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let convert_multiple_id = convert_multiple.id().clone();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Err(error) = menu.append(&convert_multiple) {
        tracing::warn!(%error, "the menu bar menu could not be built; running without one");
        return None;
    }
    let quit = MenuItem::new("Quit ToskLight Pixel", true, None);
    let quit_id = quit.id().clone();
    if let Err(error) = menu.append(&quit) {
        tracing::warn!(%error, "the menu bar menu could not be built; running without one");
        return None;
    }

    // The handler rather than a polled channel: a click arrives on the platform's own thread, and
    // `about_to_wait` already observes the shutdown it requests within one wake.
    let requested = shutdown.clone();
    #[cfg(target_os = "macos")]
    let data_directory = data_directory.map(std::path::Path::to_path_buf);
    #[cfg(not(target_os = "macos"))]
    let _ = data_directory;
    #[cfg(target_os = "windows")]
    let administration_endpoint = administration_endpoint.to_owned();
    #[cfg(target_os = "macos")]
    let administration_endpoint = administration_endpoint.to_owned();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        #[cfg(target_os = "macos")]
        if event.id == open_folder_id {
            if let Some(path) = data_directory.as_deref()
                && let Err(error) = crate::startup::open_data_directory(path)
            {
                tracing::error!(%error, "the portable Media Server folder could not be opened");
            }
            return;
        }
        #[cfg(target_os = "windows")]
        if event.id == open_pixel_id {
            if let Err(error) = open_administration(&administration_endpoint) {
                tracing::error!(%error, "the ToskLight Pixel administration interface could not be opened");
            }
            return;
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if event.id == open_settings_id {
            if let Err(error) = open_settings_in_browser(&administration_endpoint) {
                tracing::error!(%error, "the ToskLight Pixel settings could not be opened");
            }
            return;
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if event.id == convert_multiple_id {
            bulk_import.prompt();
            return;
        }
        handle(&event.id, &quit_id, &requested);
    }));

    let built = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("ToskLight Pixel")
        .with_icon(icon)
        .build();
    match built {
        Ok(icon) => {
            tracing::info!("menu bar item shown");
            Some(Tray { _icon: icon })
        }
        Err(error) => {
            tracing::warn!(%error, "the menu bar icon could not be shown; running without one");
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn administration_url(endpoint: &str) -> String {
    format!("http://{endpoint}")
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn settings_url(endpoint: &str) -> String {
    format!("http://{endpoint}/settings")
}

#[cfg(target_os = "windows")]
fn open_administration(endpoint: &str) -> std::io::Result<()> {
    let url = administration_url(endpoint);
    let mut command = std::process::Command::new("rundll32.exe");
    command
        .args(["url.dll,FileProtocolHandler", &url])
        .creation_flags(CREATE_NO_WINDOW);
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "the browser launcher exited with {status}"
        )))
    }
}

#[cfg(target_os = "windows")]
fn open_settings_in_browser(endpoint: &str) -> std::io::Result<()> {
    let url = settings_url(endpoint);
    let mut command = std::process::Command::new("rundll32.exe");
    command
        .args(["url.dll,FileProtocolHandler", &url])
        .creation_flags(CREATE_NO_WINDOW);
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "the browser launcher exited with {status}"
        )))
    }
}

#[cfg(target_os = "macos")]
fn open_settings_in_browser(endpoint: &str) -> std::io::Result<()> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg(settings_url(endpoint))
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "the browser launcher exited with {status}"
        )))
    }
}

/// Act on one menu event.
///
/// Split out of the handler so the decision it makes is testable without a menu bar to click.
fn handle(clicked: &muda::MenuId, quit: &muda::MenuId, shutdown: &Shutdown) {
    if clicked != quit {
        return;
    }
    tracing::info!("quit requested from the menu bar");
    shutdown.request(ShutdownReason::Requested);
}

/// Decode the compiled-in application icon and reduce it to menu bar size.
fn icon() -> anyhow::Result<Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(ICON));
    let mut reader = decoder.read_info()?;
    let mut source = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut source)?;
    anyhow::ensure!(
        info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight,
        "the application icon is not 8-bit RGBA"
    );
    let scaled = box_filter(&source, info.width, info.height, ICON_EDGE);
    Icon::from_rgba(scaled, ICON_EDGE, ICON_EDGE).map_err(Into::into)
}

/// Average each destination pixel over the source block it covers.
///
/// A menu bar icon is a twentieth of the artwork's size, so sampling single pixels would drop most
/// of the image and alias the LED wall into moiré. Averaging is the cheapest filter that does not.
fn box_filter(source: &[u8], width: u32, height: u32, edge: u32) -> Vec<u8> {
    let mut destination = vec![0u8; (edge * edge * 4) as usize];
    for y in 0..edge {
        for x in 0..edge {
            let from_x = x * width / edge;
            let to_x = ((x + 1) * width / edge).max(from_x + 1).min(width);
            let from_y = y * height / edge;
            let to_y = ((y + 1) * height / edge).max(from_y + 1).min(height);
            let mut totals = [0u32; 4];
            let mut counted = 0u32;
            for source_y in from_y..to_y {
                for source_x in from_x..to_x {
                    let at = ((source_y * width + source_x) * 4) as usize;
                    for channel in 0..4 {
                        totals[channel] += u32::from(source[at + channel]);
                    }
                    counted += 1;
                }
            }
            let at = ((y * edge + x) * 4) as usize;
            for channel in 0..4 {
                destination[at + channel] = (totals[channel] / counted.max(1)) as u8;
            }
        }
    }
    destination
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn the_finder_action_uses_the_operator_label() {
        assert_eq!(OPEN_FOLDER_LABEL, "open Folder in Finder");
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn the_bulk_conversion_action_uses_the_operator_label() {
        assert_eq!(CONVERT_MULTIPLE_LABEL, "Convert multiple files");
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn the_settings_action_uses_the_operator_label_and_route() {
        assert_eq!(OPEN_SETTINGS_LABEL, "Open Settings in Browser");
        assert_eq!(
            settings_url("127.0.0.1:8080"),
            "http://127.0.0.1:8080/settings"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn the_administration_action_uses_the_operator_label_and_address() {
        assert_eq!(OPEN_PIXEL_LABEL, "Open ToskLight Pixel");
        assert_eq!(
            administration_url("127.0.0.1:8080"),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn the_quit_item_stops_the_server() {
        let shutdown = Shutdown::new();
        let quit = muda::MenuId::new("quit");

        handle(&quit, &quit, &shutdown);

        assert_eq!(
            shutdown.reason(),
            Some(ShutdownReason::Requested),
            "clicking Quit has to stop the server"
        );
    }

    #[test]
    fn another_item_leaves_the_server_running() {
        // The handler is global: every menu in the process sees every click, so an item this menu
        // does not own must pass through it untouched.
        let shutdown = Shutdown::new();

        handle(
            &muda::MenuId::new("something-else"),
            &muda::MenuId::new("quit"),
            &shutdown,
        );

        assert_eq!(shutdown.reason(), None);
    }

    #[test]
    fn the_application_icon_reduces_to_menu_bar_size() {
        let decoder = png::Decoder::new(std::io::Cursor::new(ICON));
        let mut reader = decoder.read_info().expect("the icon decodes");
        let mut source = vec![0; reader.output_buffer_size().unwrap_or_default()];
        let info = reader
            .next_frame(&mut source)
            .expect("the icon has a frame");

        let scaled = box_filter(&source, info.width, info.height, ICON_EDGE);

        assert_eq!(scaled.len(), (ICON_EDGE * ICON_EDGE * 4) as usize);
        assert!(
            scaled.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0),
            "a fully transparent icon would be an empty menu bar item"
        );
    }

    #[test]
    fn reducing_averages_rather_than_samples() {
        // Two rows of one colour and two of another reduce to one pixel of their average, which a
        // sampling filter could not produce.
        let source: Vec<u8> = (0..4)
            .flat_map(|row| {
                let value = if row < 2 { 0u8 } else { 200 };
                (0..4).flat_map(move |_| [value, value, value, 255])
            })
            .collect();

        let scaled = box_filter(&source, 4, 4, 1);

        assert_eq!(scaled, vec![100, 100, 100, 255]);
    }
}
