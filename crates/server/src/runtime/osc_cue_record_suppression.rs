use super::*;

const RELEASE_TTL: Duration = Duration::from_secs(30);
const CONTINUOUS_IDLE: Duration = Duration::from_millis(650);
const ENTRY_LIMIT: usize = 1_024;

#[derive(Default)]
pub(super) struct OscCueRecordSuppression {
    entries: HashMap<SuppressionKey, Suppression>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct SuppressionKey {
    session_id: SessionId,
    source: Option<SocketAddr>,
    address: String,
}

#[derive(Clone, Copy)]
enum Suppression {
    AwaitRelease { expires_at: Instant },
    Continuous { idle_until: Instant },
}

impl OscCueRecordSuppression {
    pub(super) fn suppresses_input(
        &mut self,
        input: OscSuppressionInput<'_>,
        now: Instant,
    ) -> bool {
        self.prune(now);
        let key = input.key();
        if input.continuous {
            return self.refresh_continuous(&key, now);
        }
        if input.pressed {
            self.entries.remove(&key);
            return false;
        }
        matches!(
            self.entries.remove(&key),
            Some(Suppression::AwaitRelease { .. })
        )
    }

    pub(super) fn remember_intercept(&mut self, input: OscSuppressionInput<'_>, now: Instant) {
        let suppression = if input.continuous {
            Suppression::Continuous {
                idle_until: now + CONTINUOUS_IDLE,
            }
        } else {
            Suppression::AwaitRelease {
                expires_at: now + RELEASE_TTL,
            }
        };
        self.insert(input.key(), suppression, now);
    }

    pub(super) fn remove_source(&mut self, session_id: SessionId, source: SocketAddr) {
        self.entries
            .retain(|key, _| (key.session_id, key.source) != (session_id, Some(source)));
    }

    pub(super) fn remove_session(&mut self, session_id: SessionId) {
        self.entries.retain(|key, _| key.session_id != session_id);
    }

    fn refresh_continuous(&mut self, key: &SuppressionKey, now: Instant) -> bool {
        let Some(Suppression::Continuous { idle_until }) = self.entries.get_mut(key) else {
            return false;
        };
        *idle_until = now + CONTINUOUS_IDLE;
        true
    }

    fn insert(&mut self, key: SuppressionKey, suppression: Suppression, now: Instant) {
        self.prune(now);
        if self.entries.len() >= ENTRY_LIMIT {
            self.remove_earliest();
        }
        self.entries.insert(key, suppression);
    }

    fn prune(&mut self, now: Instant) {
        self.entries
            .retain(|_, suppression| suppression.deadline() > now);
    }

    fn remove_earliest(&mut self) {
        let key = self
            .entries
            .iter()
            .min_by_key(|(_, suppression)| suppression.deadline())
            .map(|(key, _)| key.clone());
        if let Some(key) = key {
            self.entries.remove(&key);
        }
    }
}

impl Suppression {
    const fn deadline(self) -> Instant {
        match self {
            Self::AwaitRelease { expires_at } => expires_at,
            Self::Continuous { idle_until } => idle_until,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct OscSuppressionInput<'a> {
    pub(super) session_id: SessionId,
    pub(super) source: Option<SocketAddr>,
    pub(super) address: &'a str,
    pub(super) continuous: bool,
    pub(super) pressed: bool,
}

impl OscSuppressionInput<'_> {
    fn key(self) -> SuppressionKey {
        SuppressionKey {
            session_id: self.session_id,
            source: self.source,
            address: self.address.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(
        session_id: SessionId,
        source: SocketAddr,
        address: &'a str,
        continuous: bool,
        pressed: bool,
    ) -> OscSuppressionInput<'a> {
        OscSuppressionInput {
            session_id,
            source: Some(source),
            address,
            continuous,
            pressed,
        }
    }

    #[test]
    fn momentary_release_is_consumed_but_a_new_press_retires_a_lost_release() {
        let session = SessionId::new();
        let source = "127.0.0.1:9001".parse().unwrap();
        let now = Instant::now();
        let mut state = OscCueRecordSuppression::default();
        let press = input(session, source, "/button/3", false, true);
        let release = input(session, source, "/button/3", false, false);

        state.remember_intercept(press, now);
        assert!(state.suppresses_input(release, now));
        state.remember_intercept(press, now);
        assert!(!state.suppresses_input(press, now));
        assert!(!state.suppresses_input(release, now));
    }

    #[test]
    fn continuous_samples_suppress_one_gesture_until_the_idle_window_expires() {
        let session = SessionId::new();
        let source = "127.0.0.1:9002".parse().unwrap();
        let now = Instant::now();
        let mut state = OscCueRecordSuppression::default();
        let sample = input(session, source, "/fader", true, true);

        state.remember_intercept(sample, now);
        assert!(state.suppresses_input(sample, now + Duration::from_millis(600)));
        assert!(state.suppresses_input(sample, now + Duration::from_millis(1_200)));
        assert!(!state.suppresses_input(sample, now + Duration::from_millis(1_851)));
    }

    #[test]
    fn source_and_session_cleanup_bound_hardware_suppression_authority() {
        let session = SessionId::new();
        let first = "127.0.0.1:9003".parse().unwrap();
        let second = "127.0.0.1:9004".parse().unwrap();
        let now = Instant::now();
        let mut state = OscCueRecordSuppression::default();

        state.remember_intercept(input(session, first, "/button", false, true), now);
        assert!(!state.suppresses_input(input(session, second, "/button", false, false), now,));
        state.remove_source(session, first);
        assert!(state.entries.is_empty());
        state.remember_intercept(input(session, first, "/button", false, true), now);
        state.remove_session(session);
        assert!(state.entries.is_empty());
    }
}
