//! Telling long-running listeners that an accepted edit changed what they should be doing.
//!
//! Every accepted configuration edit bumps one generation. A follower — the DMX ingress, the
//! Speed Group listener — wakes, reads the published configuration, rebinds whatever it has to,
//! and reports the generation it has caught up with. An edit route can then wait for every
//! follower to settle before it answers, so a listener that could not bind is reported in the
//! same response as the edit that moved it.
//!
//! Which settings apply live, and why the rest wait for a restart:
//!
//! | Setting | Applies | Why |
//! | --- | --- | --- |
//! | DMX protocol, universe, start address | live | ingress routing is a table swap; a protocol or sACN universe change rebinds that one UDP listener |
//! | Art-Net and sACN listen addresses | live | receive-only UDP sockets; rebinding drops at most a few frames |
//! | Speed Group listen address | live | receive-only UDP socket |
//! | Same-computer preset | live for the UDP listeners above, restart for CITP and HTTP | see those rows |
//! | Tempo source, Speed Group | live | read every frame |
//! | Pixel map zones, routes, handoffs, regions | live | read every frame; handoff inputs rebind DMX ingress |
//! | Audio gain, sensitivity, EQ | live | analysis tuning is swapped atomically |
//! | Clip switch hold, server time offset | live | read on use |
//! | CITP listen address | restart | consoles hold TCP sessions and discovery announces the port they connected to |
//! | HTTP listen address | restart | the listener serves the page making the edit |
//! | Output target, monitor, full-screen, resolution, presentation | restart | the window, GPU surface and frame clock are created when the output opens |
//! | Sound output device | restart | the audio device stream is opened once |
//! | Audio input device | restart | the capture stream is opened once |
//! | Personality | restart | layer state, render slots and the CITP/MSEX layer list consoles cached are sized when the output opens |
//! | Media library directory | restart | catalog, importer, model store and watchers are rooted at startup |
//! | Media and configuration folder | restarts itself | a different configuration is a different process state |

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;

/// How long an edit waits for listeners before answering anyway.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

/// The generation every follower is asked to reach.
#[derive(Clone)]
pub struct LiveSettings {
    requested: Arc<watch::Sender<u64>>,
    applied: Arc<Mutex<Vec<watch::Receiver<u64>>>>,
}

impl Default for LiveSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveSettings {
    pub fn new() -> Self {
        Self {
            requested: Arc::new(watch::channel(0).0),
            applied: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Says that the published configuration changed.
    pub fn changed(&self) {
        self.requested.send_modify(|generation| *generation += 1);
    }

    /// A listener that follows the published configuration.
    pub fn follow(&self) -> Follower {
        let current = *self.requested.borrow();
        let (applied, receiver) = watch::channel(current);
        self.applied
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(receiver);
        Follower {
            requested: self.requested.subscribe(),
            applied,
        }
    }

    /// [`Self::settled`], in the shape the API takes.
    pub fn settle(&self) -> media_http::SettleConfiguration {
        let live = self.clone();
        Arc::new(move || {
            let live = live.clone();
            Box::pin(async move { live.settled().await })
        })
    }

    /// Waits until every follower has caught up with the latest change, or briefly gives up.
    pub async fn settled(&self) {
        let target = *self.requested.borrow();
        let followers = self
            .applied
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        for mut follower in followers {
            let _ = tokio::time::timeout(
                SETTLE_TIMEOUT,
                follower.wait_for(|generation| *generation >= target),
            )
            .await;
        }
    }
}

/// One listener's side of [`LiveSettings`].
pub struct Follower {
    requested: watch::Receiver<u64>,
    applied: watch::Sender<u64>,
}

impl Follower {
    /// The next generation to apply, or `None` once nobody can change the configuration.
    pub async fn next(&mut self) -> Option<u64> {
        self.requested.changed().await.ok()?;
        Some(*self.requested.borrow_and_update())
    }

    /// Records that `generation` is now in effect.
    pub fn applied(&self, generation: u64) {
        self.applied.send_replace(generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_edit_waits_for_its_follower_and_no_longer() {
        let live = LiveSettings::new();
        let mut follower = live.follow();
        live.changed();
        let waiting = tokio::spawn({
            let live = live.clone();
            async move { live.settled().await }
        });
        let generation = follower.next().await.unwrap();
        assert!(!waiting.is_finished());
        follower.applied(generation);
        tokio::time::timeout(Duration::from_millis(500), waiting)
            .await
            .expect("settled once the follower applied the change")
            .unwrap();
    }

    #[tokio::test]
    async fn nothing_to_follow_is_settled_at_once() {
        let live = LiveSettings::new();
        live.changed();
        tokio::time::timeout(Duration::from_millis(100), live.settled())
            .await
            .unwrap();
    }
}
