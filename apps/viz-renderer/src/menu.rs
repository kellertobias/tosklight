//! The application menu bar.
//!
//! macOS expects an application to own a menu bar, and an operator expects **File → Open Show
//! File** to be there rather than only behind a keyboard shortcut. The menu is built once at
//! startup and its selections are drained on the main thread each frame, which is where the
//! platform requires a file dialog to be opened from.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

/// What the operator picked from the menu bar this frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuCommand {
    OpenShowFile,
    OpenRigEditor,
    CloseShowFile,
    ConnectToDesk,
    TakeSnapshot,
    QuickSettings,
}

/// The built menu bar, kept alive for as long as the application runs.
pub struct ApplicationMenu {
    #[allow(dead_code)]
    menu: Menu,
    open: MenuId,
    editor: MenuId,
    close: MenuId,
    desk: MenuId,
    snapshot: MenuId,
    settings: MenuId,
}

impl ApplicationMenu {
    /// Build and install the menu bar. A platform without one is not an error: the shortcuts
    /// still work, so the application simply runs without a menu.
    pub fn install() -> Option<Self> {
        let menu = Menu::new();

        let open = MenuItem::new(
            "Open Show File\u{2026}",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::KeyO)),
        );
        let close = MenuItem::new("Close Show File", true, None);
        let editor = MenuItem::new("Open Rig Editor…", true, None);
        let desk = MenuItem::new("Connect to Lighting Desk", true, None);
        let snapshot = MenuItem::new(
            "Take Snapshot",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::KeyS)),
        );
        let settings = MenuItem::new(
            "Quick Settings\u{2026}",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
        );

        let application = Submenu::new("ToskLight PreViz", true);
        application
            .append_items(&[
                &settings,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ])
            .ok()?;

        let file = Submenu::new("File", true);
        file.append_items(&[
            &open,
            &editor,
            &close,
            &PredefinedMenuItem::separator(),
            &snapshot,
            &PredefinedMenuItem::separator(),
            &desk,
        ])
        .ok()?;

        let window = Submenu::new("Window", true);
        window
            .append_items(&[
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::fullscreen(None),
            ])
            .ok()?;

        menu.append_items(&[&application, &file, &window]).ok()?;

        #[cfg(target_os = "macos")]
        menu.init_for_nsapp();

        Some(Self {
            menu,
            open: open.id().clone(),
            editor: editor.id().clone(),
            close: close.id().clone(),
            desk: desk.id().clone(),
            snapshot: snapshot.id().clone(),
            settings: settings.id().clone(),
        })
    }

    /// Take whatever the operator picked since the last frame.
    pub fn drain(&self) -> Vec<MenuCommand> {
        let mut commands = Vec::new();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            commands.extend(self.command_for(&event));
        }
        commands
    }

    fn command_for(&self, event: &MenuEvent) -> Option<MenuCommand> {
        if event.id == self.open {
            Some(MenuCommand::OpenShowFile)
        } else if event.id == self.editor {
            Some(MenuCommand::OpenRigEditor)
        } else if event.id == self.close {
            Some(MenuCommand::CloseShowFile)
        } else if event.id == self.desk {
            Some(MenuCommand::ConnectToDesk)
        } else if event.id == self.snapshot {
            Some(MenuCommand::TakeSnapshot)
        } else if event.id == self.settings {
            Some(MenuCommand::QuickSettings)
        } else {
            None
        }
    }
}
