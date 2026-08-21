//! Which consoles are attached, so the log describes consoles rather than TCP connections.
//!
//! A ToskLight desk answers most operator requests by opening one short-lived CITP connection,
//! which would otherwise write a connected/disconnected pair into the log for every thumbnail,
//! preview, or inspection. Presence is therefore tracked per console address: the first
//! connection announces the console, further connections are silent, and the console counts as
//! gone only after it has had no connection at all for a short settling period.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

/// How long a console may have no open connection before it is reported as disconnected.
pub(crate) const PRESENCE_SETTLE: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Default)]
struct Presence {
    connections: usize,
    announced: bool,
    generation: u64,
}

#[derive(Clone, Default)]
pub(crate) struct ConsolePresence {
    consoles: Arc<Mutex<HashMap<IpAddr, Presence>>>,
}

impl ConsolePresence {
    /// Records one accepted connection. Returns true when this console is newly present.
    pub(crate) fn arrived(&self, console: IpAddr) -> bool {
        let Ok(mut consoles) = self.consoles.lock() else {
            return false;
        };
        let presence = consoles.entry(console).or_default();
        presence.connections += 1;
        presence.generation = presence.generation.wrapping_add(1);
        if presence.announced {
            return false;
        }
        presence.announced = true;
        true
    }

    /// Records one closed connection. Returns the generation to settle against when the console
    /// has no connection left, so a reconnection during the settling period cancels the report.
    pub(crate) fn departed(&self, console: IpAddr) -> Option<u64> {
        let mut consoles = self.consoles.lock().ok()?;
        let presence = consoles.get_mut(&console)?;
        presence.connections = presence.connections.saturating_sub(1);
        (presence.connections == 0).then_some(presence.generation)
    }

    /// Returns true when the console is still gone after the settling period and was announced,
    /// which makes this the one report of its disconnection.
    pub(crate) fn settled(&self, console: IpAddr, generation: u64) -> bool {
        let Ok(mut consoles) = self.consoles.lock() else {
            return false;
        };
        let Some(presence) = consoles.get_mut(&console) else {
            return false;
        };
        if presence.connections > 0 || presence.generation != generation || !presence.announced {
            return false;
        }
        consoles.remove(&console);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn console(last: u8) -> IpAddr {
        IpAddr::from([127, 0, 0, last])
    }

    #[test]
    fn one_console_is_announced_once_however_many_connections_it_opens() {
        let presence = ConsolePresence::default();
        assert!(presence.arrived(console(1)));
        assert!(!presence.arrived(console(1)));
        assert!(presence.departed(console(1)).is_none());
        let generation = presence.departed(console(1)).expect("no connection left");
        assert!(presence.settled(console(1), generation));
        // The next connection of the same console is a new arrival worth reporting.
        assert!(presence.arrived(console(1)));
    }

    #[test]
    fn a_reconnection_inside_the_settling_period_keeps_the_console_present() {
        let presence = ConsolePresence::default();
        assert!(presence.arrived(console(1)));
        let generation = presence.departed(console(1)).expect("no connection left");
        assert!(!presence.arrived(console(1)));
        assert!(!presence.settled(console(1), generation));
        assert!(!presence.arrived(console(1)));
    }

    #[test]
    fn consoles_are_tracked_separately_by_address() {
        let presence = ConsolePresence::default();
        assert!(presence.arrived(console(1)));
        assert!(presence.arrived(console(2)));
        let generation = presence.departed(console(2)).expect("no connection left");
        assert!(presence.settled(console(2), generation));
        assert!(!presence.arrived(console(1)));
    }
}
