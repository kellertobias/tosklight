use crate::{hardware_controls, lifecycle};

pub(crate) fn handle_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "open-hardware-controls" => {
            if let Err(error) = hardware_controls::open() {
                eprintln!("failed to open Hardware Controls: {error}");
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
    let tools = tauri::menu::SubmenuBuilder::new(app, "Tools")
        .item(&open)
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
