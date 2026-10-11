//! Captured source ownership for delayed queue execution; the engine retains its show policies.
use super::*;
#[derive(Clone)]
pub(in crate::runtime) struct CapturedPreloadPlaybackContext {
    fallback_desk: Uuid,
}
impl CapturedPreloadPlaybackContext {
    pub(in crate::runtime) fn new(fallback_desk: Uuid) -> Self {
        Self { fallback_desk }
    }
    pub(in crate::runtime) fn commands(
        &self,
        pending: &[light_programmer::PreloadPlaybackAction],
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<light_engine::PlaybackBatchCommand>, String> {
        let mut commands = preload_batch_commands(pending)?;
        for (pending, command) in pending.iter().zip(&mut commands) {
            let desk_id = pending.origin_desk_id.unwrap_or(self.fallback_desk);
            command.control_source = pending.control_source.scoped_to_desk(desk_id)?;
            command.activation_origin = Some(light_playback::PlaybackActivationOrigin {
                at,
                desk_id: Some(desk_id),
                surface: if pending.control_source.surface_id.starts_with("hardware:")
                    || pending.control_source.surface_id.starts_with("osc:")
                {
                    light_playback::PlaybackActivationSurface::Osc
                } else {
                    light_playback::PlaybackActivationSurface::Physical
                },
                exclusion_scope: light_playback::PlaybackExclusionScope::Show,
            });
        }
        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_queue_keeps_exact_address_source_and_origin_but_uses_evaluation_time() {
        let fallback = Uuid::new_v4();
        let authored = Uuid::new_v4();
        let context = CapturedPreloadPlaybackContext::new(fallback);
        let action = light_programmer::PreloadPlaybackAction {
            address: light_playback::PlaybackAddress::new(1, 401).unwrap(),
            origin_desk_id: None,
            action: light_programmer::PreloadPlaybackQueueAction::On,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "pane:busking".into(),
                control_id: 7,
            },
        };
        let mut later = action.clone();
        later.action = light_programmer::PreloadPlaybackQueueAction::Go;
        later.origin_desk_id = Some(authored);
        later.control_source.surface_id = "hardware:attached".into();
        let queue = Arc::new(vec![action, later]);
        let captured = Arc::clone(&queue);
        let first_at = chrono::DateTime::from_timestamp_millis(1000).unwrap();
        let later_at = chrono::DateTime::from_timestamp_millis(2000).unwrap();
        let first = context.commands(&captured, first_at).unwrap();
        let delayed = context.commands(&captured, later_at).unwrap();
        assert_eq!(delayed.len(), 2);
        assert_eq!(delayed[0].action, PlaybackBatchAction::On);
        assert_eq!(delayed[1].action, PlaybackBatchAction::Go);
        for (index, command) in delayed.iter().enumerate() {
            assert_eq!(command.address, queue[index].address);
            assert_eq!(
                command.control_source,
                queue[index]
                    .control_source
                    .scoped_to_desk(if index == 0 { fallback } else { authored })
                    .unwrap()
            );
            let origin = command.activation_origin.as_ref().unwrap();
            assert_eq!(origin.at, later_at);
            assert_eq!(
                origin.desk_id,
                Some(if index == 0 { fallback } else { authored })
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
        let fresh = CapturedPreloadPlaybackContext::new(Uuid::new_v4())
            .commands(&queue, later_at)
            .unwrap();
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
