//! Immutable queue environment. Delayed evaluation must not consult a newer desk or show.
use super::*;
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::runtime) struct CapturedPreloadPlaybackContext {
    zones: Arc<[Vec<u16>]>,
    fallback_desk: Uuid,
}

impl CapturedPreloadPlaybackContext {
    pub(in crate::runtime) fn new(zones: Arc<[Vec<u16>]>, fallback_desk: Uuid) -> Self {
        Self {
            zones,
            fallback_desk,
        }
    }

    /// Caller supplies the queue captured alongside this context. Timing belongs to the
    /// evaluation boundary; authored desk identities and global zone membership stay captured.
    pub(in crate::runtime) fn commands(
        &self,
        pending: &[light_programmer::PreloadPlaybackAction],
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<light_engine::PlaybackBatchCommand>, String> {
        let mut commands = preload_batch_commands(pending)?;
        for (pending, command) in pending.iter().zip(&mut commands) {
            let applies = self
                .zones
                .iter()
                .any(|zone| zone.contains(&pending.playback_number));
            command.exclusion_zones = Arc::clone(&self.zones);
            command.activation_origin = Some(light_playback::PlaybackActivationOrigin {
                at,
                desk_id: Some(pending.origin_desk_id.unwrap_or(self.fallback_desk)),
                surface: activation_surface(pending.surface),
                exclusion_scope: if applies {
                    light_playback::PlaybackExclusionScope::Show
                } else {
                    light_playback::PlaybackExclusionScope::None
                },
            });
        }
        Ok(commands)
    }
}

const fn activation_surface(
    surface: light_programmer::PreloadPlaybackQueueSurface,
) -> light_playback::PlaybackActivationSurface {
    match surface {
        light_programmer::PreloadPlaybackQueueSurface::Physical => {
            light_playback::PlaybackActivationSurface::Physical
        }
        light_programmer::PreloadPlaybackQueueSurface::Virtual => {
            light_playback::PlaybackActivationSurface::Virtual
        }
        light_programmer::PreloadPlaybackQueueSurface::Osc => {
            light_playback::PlaybackActivationSurface::Osc
        }
        light_programmer::PreloadPlaybackQueueSurface::Matter => {
            light_playback::PlaybackActivationSurface::Matter
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_queue_keeps_captured_zones_and_origins_but_uses_its_evaluation_time() {
        let fallback = Uuid::new_v4();
        let authored = Uuid::new_v4();
        let mut zones: Arc<[Vec<u16>]> = vec![vec![1_001, 1_302]].into();
        let context = CapturedPreloadPlaybackContext::new(Arc::clone(&zones), fallback);
        let action = light_programmer::PreloadPlaybackAction {
            playback_number: 1_001,
            origin_desk_id: None,
            page: Some(1),
            action: light_programmer::PreloadPlaybackQueueAction::On,
            surface: light_programmer::PreloadPlaybackQueueSurface::Virtual,
        };
        let mut later = action.clone();
        later.action = light_programmer::PreloadPlaybackQueueAction::Go;
        later.origin_desk_id = Some(authored);
        later.surface = light_programmer::PreloadPlaybackQueueSurface::Osc;
        let queue = Arc::new(vec![action, later]);
        let captured_queue = Arc::clone(&queue);
        Arc::make_mut(&mut zones)[0] = vec![1_003, 1_004];
        let new_context = CapturedPreloadPlaybackContext::new(zones, Uuid::new_v4());
        let first_at = chrono::DateTime::from_timestamp_millis(1_000).unwrap();
        let later_at = chrono::DateTime::from_timestamp_millis(2_000).unwrap();
        let first = context.commands(&captured_queue, first_at).unwrap();
        let delayed = context.commands(&captured_queue, later_at).unwrap();
        assert_eq!(delayed.len(), 2);
        assert_eq!(delayed[0].action, PlaybackBatchAction::On);
        assert_eq!(delayed[1].action, PlaybackBatchAction::Go);
        for (index, command) in delayed.iter().enumerate() {
            assert_eq!(&*command.exclusion_zones, &[vec![1_001, 1_302]]);
            let origin = command.activation_origin.as_ref().unwrap();
            assert_eq!(origin.at, later_at);
            assert_eq!(
                origin.desk_id,
                Some(if index == 0 { fallback } else { authored })
            );
            assert_eq!(
                origin.exclusion_scope,
                light_playback::PlaybackExclusionScope::Show
            );
            assert_eq!(
                first[index].activation_origin.as_ref().unwrap().at,
                first_at
            );
        }
        assert_eq!(
            delayed[1].activation_origin.as_ref().unwrap().surface,
            light_playback::PlaybackActivationSurface::Osc
        );
        let fresh = new_context.commands(&queue, later_at).unwrap();
        assert_eq!(
            fresh[0].activation_origin.as_ref().unwrap().exclusion_scope,
            light_playback::PlaybackExclusionScope::None
        );
        assert_ne!(
            fresh[0].activation_origin.as_ref().unwrap().desk_id,
            Some(fallback)
        );
        assert_eq!(
            fresh[1].activation_origin.as_ref().unwrap().desk_id,
            Some(authored)
        );
    }
}
