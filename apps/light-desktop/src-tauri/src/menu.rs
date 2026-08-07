use crate::{hardware_controls, lifecycle, visualizer::Visualizer};
use tauri::Manager;

pub(crate) fn handle_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "open-hardware-controls" => {
            if let Err(error) = hardware_controls::open() {
                eprintln!("failed to open Hardware Controls: {error}");
            }
        }
        // The Stage view drawn by the native renderer instead of the web one. It opens as the
        // desk's own supervised window rather than a second application — the embedded pane is a
        // later step, and this is the arrangement the plan makes first.
        "open-visualizer" => {
            if let Err(error) = app.state::<Visualizer>().open() {
                eprintln!("failed to open the visualizer: {error}");
            }
        }
        "close-visualizer" => {
            if let Err(error) = app.state::<Visualizer>().close() {
                eprintln!("failed to close the visualizer: {error}");
            }
        }
        "quit" => lifecycle::request_quit(app),
        _ => {}
    }
}

pub(crate) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    let open =
        tauri::menu::MenuItemBuilder::with_id("open-hardware-controls", "Open Hardware Controls")
            .build(app)?;
    let open_visualizer =
        tauri::menu::MenuItemBuilder::with_id("open-visualizer", "Open Stage in the Visualizer")
            .build(app)?;
    let close_visualizer =
        tauri::menu::MenuItemBuilder::with_id("close-visualizer", "Close the Visualizer")
            .build(app)?;
    let tools = tauri::menu::SubmenuBuilder::new(app, "Tools")
        .item(&open)
        .separator()
        .item(&open_visualizer)
        .item(&close_visualizer)
        .build()?;
    let menu = tauri::menu::Menu::default(app.handle())?;
    // Swap the predefined Quit item (last entry of the macOS app submenu) for one we can
    // intercept to confirm.
    #[cfg(target_os = "macos")]
    if let Some(tauri::menu::MenuItemKind::Submenu(app_menu)) = menu.items()?.into_iter().next() {
        if let Some(native_quit) = app_menu.items()?.last() {
            app_menu.remove(native_quit)?;
        }
        app_menu.append(
            &tauri::menu::MenuItemBuilder::with_id("quit", "Quit ToskLight")
                .accelerator("CmdOrCtrl+Q")
                .build(app)?,
        )?;
    }
    menu.append(&tools)?;
    app.set_menu(menu)?;
    Ok(())
}
