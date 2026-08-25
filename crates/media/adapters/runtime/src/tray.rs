//! The Media Server's presence on the desktop: a menu bar item on macOS, a notification-area item
//! on Windows.
//!
//! A media server has no main window. Its outputs are full-screen surfaces on the monitors an
//! operator assigned, and with none assigned it draws nothing at all — so without this the process
//! is invisible, and the only way to stop it is Activity Monitor.
//!
//! The menu is the icon and Quit, deliberately. Everything an operator can change lives in the
//! administration interface; a menu that mirrors any of it becomes a second surface to keep in
//! sync with the first.

use crate::shutdown::{Shutdown, ShutdownReason};
use muda::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// The application icon, compiled in rather than read from the bundle: the same executable runs
/// bundled and bare, and an icon that only appears in one of them is a difference nobody wants to
/// discover during a show.
const ICON: &[u8] = include_bytes!("../../../../../assets/branding/ToskLight Pixel.png");

/// What the menu bar draws. Big enough for a Retina menu bar, small enough that decoding it costs
/// nothing at startup.
const ICON_EDGE: u32 = 44;

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
pub fn show(shutdown: &Shutdown) -> Option<Tray> {
    let icon = match icon() {
        Ok(icon) => icon,
        Err(error) => {
            tracing::warn!(%error, "the menu bar icon could not be decoded; running without one");
            return None;
        }
    };

    let menu = Menu::new();
    let quit = MenuItem::new("Quit ToskLight Media", true, None);
    let quit_id = quit.id().clone();
    if let Err(error) = menu.append(&quit) {
        tracing::warn!(%error, "the menu bar menu could not be built; running without one");
        return None;
    }

    // The handler rather than a polled channel: a click arrives on the platform's own thread, and
    // `about_to_wait` already observes the shutdown it requests within one wake.
    let requested = shutdown.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        handle(&event.id, &quit_id, &requested);
    }));

    let built = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("ToskLight Media")
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
