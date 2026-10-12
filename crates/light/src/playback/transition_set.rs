use super::{
    PlaybackAction, PlaybackCommand, PlaybackPorts, PlaybackRelatedResult, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackShowScope, ResolvedPlaybackAddress,
    committed_playback_event,
};
use crate::{ActionContext, ActionEnvelope, ActionError, ActionErrorKind, EventBus};
use std::{cmp::Ordering, collections::HashMap};

pub(super) struct RelatedTransitionSet {
    identities: Vec<PlaybackRuntimeIdentity>,
    before: HashMap<PlaybackRuntimeIdentity, PlaybackRuntimeProjection>,
    scope: PlaybackShowScope,
}

impl RelatedTransitionSet {
    pub(super) fn capture(
        envelope: &ActionEnvelope<PlaybackCommand>,
        resolved: ResolvedPlaybackAddress,
        primary: PlaybackRuntimeIdentity,
        scope: PlaybackShowScope,
        ports: &dyn PlaybackPorts,
    ) -> Result<Self, ActionError> {
        let mut identities = ports.related_runtime_identities(
            &envelope.context,
            resolved,
            envelope.command.action.clone(),
            &envelope.command.control_source,
        )?;
        identities.retain(|identity| identity != &primary);
        identities.sort_by(compare_identity);
        identities.dedup();
        // These identities are server-derived and already unique. The public repair
        // request limit must not cap atomic fanout across valid Solo members/aliases.
        let before = exact_projections(&envelope.context, &identities, scope, ports)?;
        Ok(Self {
            identities,
            before,
            scope,
        })
    }

    pub(super) fn publish_changes(
        self,
        events: &EventBus,
        context: &ActionContext,
        ports: &dyn PlaybackPorts,
    ) -> Result<Vec<PlaybackRelatedResult>, ActionError> {
        let after = exact_projections(context, &self.identities, self.scope, ports)?;
        Ok(self
            .identities
            .into_iter()
            .filter_map(|identity| {
                publish_change(events, context, &self.before, after.get(&identity))
            })
            .collect())
    }
}

fn exact_projections(
    context: &ActionContext,
    identities: &[PlaybackRuntimeIdentity],
    scope: PlaybackShowScope,
    ports: &dyn PlaybackPorts,
) -> Result<HashMap<PlaybackRuntimeIdentity, PlaybackRuntimeProjection>, ActionError> {
    if identities.is_empty() {
        return Ok(HashMap::new());
    }
    let projections = ports.projections(context, identities)?;
    let mapped = projections
        .into_iter()
        .map(|projection| (projection.identity.clone(), projection))
        .collect::<HashMap<_, _>>();
    if mapped.len() == identities.len()
        && identities
            .iter()
            .all(|identity| mapped.contains_key(identity))
        && mapped.values().all(|projection| projection.scope == scope)
    {
        Ok(mapped)
    } else {
        Err(ActionError::new(
            ActionErrorKind::Internal,
            "playback projection port returned invalid related identities",
        ))
    }
}

fn publish_change(
    events: &EventBus,
    context: &ActionContext,
    before: &HashMap<PlaybackRuntimeIdentity, PlaybackRuntimeProjection>,
    after: Option<&PlaybackRuntimeProjection>,
) -> Option<PlaybackRelatedResult> {
    let after = after?.clone();
    let draft = committed_playback_event(
        context,
        PlaybackAction::Off { pressed: true },
        None,
        before.get(&after.identity)?.clone(),
        after.clone(),
    )?;
    Some(PlaybackRelatedResult {
        projection: after,
        event_sequence: events.publish(draft).sequence,
    })
}

fn compare_identity(left: &PlaybackRuntimeIdentity, right: &PlaybackRuntimeIdentity) -> Ordering {
    fn key(identity: &PlaybackRuntimeIdentity) -> (u8, Vec<u8>) {
        match identity {
            PlaybackRuntimeIdentity::Playback(address) => (
                0,
                vec![
                    address.page(),
                    (address.position() >> 8) as u8,
                    address.position() as u8,
                ],
            ),
            PlaybackRuntimeIdentity::CueList(id) => (1, id.0.as_bytes().to_vec()),
            PlaybackRuntimeIdentity::DirectCueList(id) => (2, id.0.as_bytes().to_vec()),
            PlaybackRuntimeIdentity::Group(id) => (3, id.as_str().as_bytes().to_vec()),
        }
    }
    key(left).cmp(&key(right))
}
