use light_playback::PlaybackRuntimeEffect;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PlaybackPersistencePlan {
    pub(super) active_playbacks: bool,
    pub(super) output_runtime: bool,
}

impl PlaybackPersistencePlan {
    pub(super) const fn none() -> Self {
        Self {
            active_playbacks: false,
            output_runtime: false,
        }
    }

    pub(super) const fn active_playbacks() -> Self {
        Self {
            active_playbacks: true,
            output_runtime: false,
        }
    }

    pub(super) const fn output_runtime() -> Self {
        Self {
            active_playbacks: false,
            output_runtime: true,
        }
    }

    pub(super) const fn for_cuelist(effect: PlaybackRuntimeEffect) -> Self {
        if effect.durable() {
            Self::active_playbacks()
        } else {
            Self::none()
        }
    }

    pub(super) const fn combine(self, other: Self) -> Self {
        Self {
            active_playbacks: self.active_playbacks || other.active_playbacks,
            output_runtime: self.output_runtime || other.output_runtime,
        }
    }

    pub(super) fn domains(self) -> impl Iterator<Item = PlaybackPersistenceDomain> {
        [
            self.active_playbacks
                .then_some(PlaybackPersistenceDomain::ActivePlaybacks),
            self.output_runtime
                .then_some(PlaybackPersistenceDomain::OutputRuntime),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlaybackPersistenceDomain {
    ActivePlaybacks,
    OutputRuntime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_schedules_no_persistence_domains() {
        assert_eq!(
            PlaybackPersistencePlan::none()
                .domains()
                .collect::<Vec<_>>(),
            Vec::<PlaybackPersistenceDomain>::new()
        );
    }

    #[test]
    fn plans_combine_without_duplicating_domain_calls() {
        let plan = PlaybackPersistencePlan::active_playbacks()
            .combine(PlaybackPersistencePlan::output_runtime())
            .combine(PlaybackPersistencePlan::active_playbacks());

        assert_eq!(
            plan.domains().collect::<Vec<_>>(),
            vec![
                PlaybackPersistenceDomain::ActivePlaybacks,
                PlaybackPersistenceDomain::OutputRuntime,
            ]
        );
    }

    #[test]
    fn single_domain_plans_never_expand_to_generic_dual_writes() {
        assert_eq!(
            PlaybackPersistencePlan::active_playbacks()
                .domains()
                .collect::<Vec<_>>(),
            vec![PlaybackPersistenceDomain::ActivePlaybacks]
        );
        assert_eq!(
            PlaybackPersistencePlan::output_runtime()
                .domains()
                .collect::<Vec<_>>(),
            vec![PlaybackPersistenceDomain::OutputRuntime]
        );
    }

    #[test]
    fn only_durable_cuelist_effects_schedule_active_playback_persistence() {
        assert_eq!(
            PlaybackPersistencePlan::for_cuelist(PlaybackRuntimeEffect::Durable),
            PlaybackPersistencePlan::active_playbacks()
        );
        for effect in [
            PlaybackRuntimeEffect::None,
            PlaybackRuntimeEffect::Transient,
        ] {
            assert_eq!(
                PlaybackPersistencePlan::for_cuelist(effect),
                PlaybackPersistencePlan::none()
            );
        }
    }
}
