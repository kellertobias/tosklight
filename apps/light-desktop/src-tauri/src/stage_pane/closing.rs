//! A closing pane owns its compositor through detachment/blanking and terminal GPU completion.
use super::*;
use std::sync::atomic::Ordering;

/// A skipped/failed clear is never permission to drop its swapchain.
fn retire_slot<T>(
    slot: &mut Option<T>,
    poll: impl FnOnce(&mut T) -> Result<bool, String>,
) -> Result<bool, String> {
    let complete = match slot.as_mut() {
        Some(value) => poll(value)?,
        None => true,
    };
    if complete {
        *slot = None;
    }
    Ok(complete)
}

impl StagePane {
    pub(super) fn assert_openable(&self) -> Result<(), String> {
        if self.retiring.load(Ordering::Acquire) {
            return Err("The previous Stage surface is still clearing; reveal this window before reopening its Stage".into());
        }
        Ok(())
    }

    pub(crate) fn close(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        self.begin_close_locked(&mut guard)?;
        self.clear_benchmark_samples()?;
        Ok(())
    }

    pub(super) fn begin_close_locked(&self, guard: &mut Option<Running>) -> Result<(), String> {
        self.drawing.store(false, Ordering::Release);
        if !self.retiring.swap(true, Ordering::AcqRel) {
            *self
                .retirement_error
                .lock()
                .map_err(|_| "the Stage retirement")? = None;
            if let Some(running) = guard.as_mut() {
                running.send(&ToHelper::Visibility { visible: false });
                running.send(&ToHelper::Shutdown);
                let helper = running.helper.take();
                let writer = running.to_helper.take();
                // Only process/pipe ownership crosses threads. No surface, GPU or window object
                // goes to this worker, and Child::wait can never block the Control UI.
                if helper.is_some() || writer.is_some() {
                    tauri::async_runtime::spawn_blocking(move || {
                        drop(writer);
                        if let Some(mut helper) = helper {
                            helper.stop();
                        }
                    });
                }
            }
        }
        let result = self.poll_close_locked(guard);
        wake_pump();
        result
    }

    pub(super) fn poll_close_locked(&self, guard: &mut Option<Running>) -> Result<(), String> {
        let result = retire_slot(guard, |running| {
            // Release the bounded reader queue without adopting another old helper image.
            for _ in 0..128 {
                if running.inbox.try_recv().is_err() {
                    break;
                }
            }
            if running.compositor.begin_retirement()? {
                return Ok(true);
            }
            running
                .compositor
                .poll_retirement(running.visibility.window)
        });
        match result {
            Ok(true) => {
                self.retiring.store(false, Ordering::Release);
                *self
                    .retirement_error
                    .lock()
                    .map_err(|_| "the Stage retirement")? = None;
                self.retirement_done.notify_waiters();
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(error) => {
                *self
                    .retirement_error
                    .lock()
                    .map_err(|_| "the Stage retirement")? = Some(error.clone());
                self.retirement_done.notify_waiters();
                Err(error)
            }
        }
    }

    pub(super) fn needs_pump(&self) -> bool {
        self.is_drawing() || self.retiring.load(Ordering::Acquire)
    }

    pub(super) fn next_poll_delay(&self) -> std::time::Duration {
        self.inner
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|r| r.compositor.next_poll_delay()))
            .unwrap_or(IDLE)
    }

    /// An async waiter consumes no Control thread and no GPU work while hidden retirement waits
    /// for the owning window to become drawable again. Errors leave the owned surface retained.
    pub(super) async fn wait_closed(&self) -> Result<(), String> {
        loop {
            let notified = self.retirement_done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.retiring.load(Ordering::Acquire) {
                return Ok(());
            }
            if let Some(error) = self
                .retirement_error
                .lock()
                .map_err(|_| "the Stage retirement")?
                .clone()
            {
                return Err(error);
            }
            notified.await;
        }
    }

    /// A destroyed window needs no clear, but dropping its last Queue can still implicitly wait.
    /// Keep the single existing retirement slot and stop only process/pipe ownership off-thread.
    pub(super) fn window_destroyed(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.visibility.window = false;
            running.compositor.begin_window_retirement();
        }
        self.begin_close_locked(&mut guard)
    }
}

pub(super) async fn close_command(window: tauri::WebviewWindow, key: String) -> Result<(), String> {
    use tauri::Manager;
    let handle = window.app_handle().clone();
    let owner = handle.clone();
    let (done, wait) = tokio::sync::oneshot::channel();
    handle
        .run_on_main_thread(move || {
            let panes = owner.state::<StagePanes>();
            let result = panes.close(&key).and_then(|()| panes.pane(&key));
            let _ = done.send(result);
        })
        .map_err(|error| error.to_string())?;
    if let Some(pane) = wait.await.map_err(|_| "The Stage close owner stopped")?? {
        pane.wait_closed().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Surface(Arc<AtomicUsize>);
    impl Drop for Surface {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn pending_or_failed_retirement_never_drops_the_surface() {
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut slot = Some(Surface(dropped.clone()));
        assert!(!retire_slot(&mut slot, |_| Ok(false)).unwrap());
        assert!(retire_slot(&mut slot, |_| Err("surface unavailable".into())).is_err());
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
        assert!(retire_slot(&mut slot, |_| Ok(true)).unwrap());
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert!(retire_slot(&mut slot, |_| panic!("already retired")).unwrap());
    }
    #[test]
    fn a_pending_retirement_blocks_replacement_allocation() {
        let pane = StagePane::default();
        assert!(pane.assert_openable().is_ok());
        pane.retiring.store(true, Ordering::Release);
        assert!(
            pane.assert_openable()
                .unwrap_err()
                .contains("still clearing")
        );
        assert!(pane.needs_pump());
        pane.retiring.store(false, Ordering::Release);
        assert!(!pane.needs_pump());
    }
}
