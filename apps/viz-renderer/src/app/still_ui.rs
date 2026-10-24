//! In-window job controls: no focus-stealing native dialogs and no GPU redraw for unchanged status.
use crate::stills::{Phase, Status, StillJobId};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
};
use viz_render::Overlay;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum Action {
    Ultra,
    Ultimate,
    Cancel,
    Open,
    Reveal,
    Close,
}
#[derive(Clone, Copy, Default)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Rect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.w && y <= self.y + self.h
    }
}
#[derive(Default)]
pub(in crate::app) struct StillUi {
    status: Option<Status>,
    dismissed: Option<StillJobId>,
    error: Option<String>,
    panel: Rect,
    actions: Vec<(Rect, Action)>,
    scroll: usize,
    file_action: Option<Receiver<Result<(), String>>>,
}
impl StillUi {
    pub(in crate::app) fn reset(&mut self) {
        self.error = None;
        self.dismissed = None;
        self.scroll = 0;
        // An opener belongs to the previous image, never to the next invocation.
        self.file_action = None;
    }
    pub(in crate::app) fn fail(&mut self, error: String) {
        self.error = Some(error);
    }
    pub(in crate::app) fn observe(&mut self, status: Option<Status>) {
        if let Some(receiver) = &self.file_action {
            match receiver.try_recv() {
                Ok(result) => {
                    self.file_action = None;
                    if let Err(error) = result {
                        self.fail(error);
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.file_action = None;
                    self.fail(
                        "Saved image opener stopped unexpectedly; retry Open or Reveal".into(),
                    );
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if status != self.status {
            self.scroll = 0;
            self.status = status;
        }
    }
    pub(in crate::app) fn visible(&self) -> bool {
        self.error.is_some()
            || self
                .status
                .as_ref()
                .is_some_and(|s| self.dismissed.as_ref() != Some(&s.job_id))
    }
    pub(in crate::app) fn modal(&self) -> bool {
        self.visible()
            && self
                .status
                .as_ref()
                .is_some_and(|s| matches!(s.phase, Phase::NeedsChoice { .. }))
    }
    pub(in crate::app) fn close(&mut self) {
        self.error = None;
        self.dismissed = self.status.as_ref().map(|s| s.job_id.clone());
    }
    pub(in crate::app) fn scroll(&mut self, delta: i32) {
        self.scroll = self.scroll.saturating_add_signed(delta as isize);
    }
    pub(in crate::app) fn contains(&self, x: f32, y: f32) -> bool {
        self.visible() && self.panel.contains(x, y)
    }
    pub(in crate::app) fn action_at(&self, x: f32, y: f32) -> Option<Action> {
        self.visible()
            .then(|| {
                self.actions
                    .iter()
                    .find_map(|(r, a)| r.contains(x, y).then_some(*a))
            })
            .flatten()
    }
    pub(in crate::app) fn open_file(&mut self, path: PathBuf, reveal: bool) {
        if self.file_action.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.file_action = Some(receiver);
        std::thread::spawn(move || {
            let result = open_command(&path, reveal)
                .status()
                .map_err(|e| format!("Cannot open saved image: {e}"))
                .and_then(|s| {
                    if s.success() {
                        Ok(())
                    } else {
                        Err(format!("Saved image opener exited with {s}"))
                    }
                });
            let _ = sender.send(result);
        });
    }
    pub(in crate::app) fn draw(&mut self, overlay: &mut Overlay, width: f32, height: f32) {
        self.actions.clear();
        if !self.visible() {
            return;
        }
        let scale = (width / 1280.0).clamp(0.75, 2.0);
        let w = (900.0 * scale).min(width - 24.0);
        let h = (520.0 * scale).min(height - 40.0);
        let x = (width - w) * 0.5;
        let y = (height - h) * 0.5;
        self.panel = Rect { x, y, w, h };
        let padding = 16.0 * scale;
        let text = [0.9, 0.94, 1.0, 1.0];
        let dim = [0.55, 0.65, 0.75, 1.0];
        if self.modal() {
            overlay.rect(0.0, 0.0, width, height, [0.0, 0.0, 0.0, 0.6]);
        }
        overlay.rect(x, y, w, h, [0.015, 0.02, 0.03, 0.98]);
        overlay.text(
            x + padding,
            y + padding,
            scale,
            text,
            "Render 4K Image — 3840 × 2160",
        );
        let (lines, buttons) = self.content();
        let chars = ((w - 2.0 * padding) / (8.0 * scale)).max(12.0) as usize;
        let lines: Vec<_> = lines.iter().flat_map(|s| wrap(s, chars)).collect();
        let line_h = Overlay::line_height(scale) * 1.3;
        let room = ((h - 116.0 * scale) / line_h).max(1.0) as usize;
        self.scroll = self.scroll.min(lines.len().saturating_sub(room));
        for (index, line) in lines.iter().skip(self.scroll).take(room).enumerate() {
            overlay.text(
                x + padding,
                y + 52.0 * scale + index as f32 * line_h,
                scale,
                text,
                line,
            );
        }
        if lines.len() > room {
            overlay.text(
                x + padding,
                y + h - 86.0 * scale,
                scale,
                dim,
                "Scroll / Up / Down for full explanation",
            );
        }
        let button_w = (w - padding * 2.0) / buttons.len().max(1) as f32;
        for (index, (label, action)) in buttons.into_iter().enumerate() {
            let rect = Rect {
                x: x + padding + index as f32 * button_w,
                y: y + h - 54.0 * scale,
                w: button_w - 8.0 * scale,
                h: 36.0 * scale,
            };
            overlay.rect(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if action.is_some() {
                    [0.06, 0.16, 0.24, 1.0]
                } else {
                    [0.04, 0.05, 0.06, 1.0]
                },
            );
            overlay.clipped_text(
                rect.x + 8.0 * scale,
                rect.y + 9.0 * scale,
                scale,
                if action.is_some() { text } else { dim },
                label,
                rect.x + rect.w - 4.0 * scale,
            );
            if let Some(action) = action {
                self.actions.push((rect, action));
            }
        }
    }
    pub(super) fn content(&self) -> (Vec<String>, Vec<(&'static str, Option<Action>)>) {
        if let Some(error) = &self.error
            && self.status.as_ref().is_none_or(|s| s.phase.is_terminal())
        {
            return (vec![error.clone()], vec![("Close", Some(Action::Close))]);
        }
        let Some(status) = &self.status else {
            return (vec![], vec![]);
        };
        let mut lines = vec![status.framing.clone()];
        if let Some(error) = &self.error {
            lines.push(error.clone());
        }
        let buttons = match &status.phase {
            Phase::NeedsChoice {
                blender_version,
                ultimate_reasons,
                ultra_explanation,
                ultimate_explanation,
            } => {
                lines.extend([
                    format!("Blender {blender_version} is available. Choose image quality:"),
                    ultra_explanation.clone(),
                    ultimate_explanation.clone(),
                ]);
                lines.extend(
                    ultimate_reasons
                        .iter()
                        .map(|r| format!("Ultimate unavailable: {r}")),
                );
                vec![
                    ("[U] Ultra · ToskLight", Some(Action::Ultra)),
                    (
                        "[B] Ultimate · Blender",
                        ultimate_reasons.is_empty().then_some(Action::Ultimate),
                    ),
                    ("[Esc] Cancel", Some(Action::Cancel)),
                ]
            }
            Phase::Saved { path } => {
                lines.extend(["3840 × 2160 image saved".into(), path.display().to_string()]);
                if let Some(engine) = status.engine {
                    lines.push(match engine {
                        crate::stills::Engine::Ultra => "Ultra · ToskLight".into(),
                        crate::stills::Engine::Ultimate => "Ultimate · Blender".into(),
                    });
                }
                vec![
                    ("Open", Some(Action::Open)),
                    ("Reveal", Some(Action::Reveal)),
                    ("Close", Some(Action::Close)),
                ]
            }
            Phase::Failed { detail } => {
                lines.push(format!("Image failed: {detail}"));
                vec![("Close", Some(Action::Close))]
            }
            Phase::Cancelled => {
                lines.push("Image cancelled".into());
                vec![("Close", Some(Action::Close))]
            }
            phase => {
                lines.push(match phase {
                    Phase::Queued => "Image queued".into(),
                    Phase::CheckingBlender => "Checking Blender availability…".into(),
                    Phase::Rendering { detail } => detail.clone(),
                    Phase::Writing => "Writing PNG…".into(),
                    Phase::Cancelling { detail } => detail.clone(),
                    _ => String::new(),
                });
                vec![(
                    "Cancel",
                    (!matches!(phase, Phase::Cancelling { .. })).then_some(Action::Cancel),
                )]
            }
        };
        if let Some(notice) = &status.notice {
            lines.push(notice.clone());
        }
        (lines, buttons)
    }
}
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for character in text.chars() {
        if character == '\n' || line.chars().count() >= width {
            lines.push(std::mem::take(&mut line));
            if character == '\n' {
                continue;
            }
        }
        line.push(character);
    }
    lines.push(line);
    lines
}
fn open_command(path: &std::path::Path, reveal: bool) -> std::process::Command {
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        if reveal {
            c.arg("-R");
        }
        c.arg(path);
        c
    }
    #[cfg(target_os = "windows")]
    {
        let mut c = std::process::Command::new("explorer.exe");
        if reveal {
            c.arg(format!("/select,{}", path.display()));
        } else {
            c.arg(path);
        }
        c
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(if reveal {
            path.parent().unwrap_or(path)
        } else {
            path
        });
        c
    }
}

#[cfg(test)]
#[path = "still_ui_tests.rs"]
mod tests;
