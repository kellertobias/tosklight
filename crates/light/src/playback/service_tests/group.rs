use super::*;
use crate::ActionContext;

struct GroupPorts {
    inner: FakePorts,
    playback: Option<u16>,
}

impl GroupPorts {
    fn new(playback: Option<u16>) -> Self {
        Self {
            inner: FakePorts::default(),
            playback,
        }
    }

    fn actions(&self) -> Vec<ObservedAction> {
        self.inner.actions()
    }

    fn projection_reads(&self) -> Vec<PlaybackRuntimeIdentity> {
        self.inner.projection_reads()
    }
}

impl PlaybackPorts for GroupPorts {
    fn current_page(&self, context: &ActionContext) -> Result<u8, ActionError> {
        self.inner.current_page(context)
    }

    fn playback_at(&self, page: u8, slot: u8) -> Result<Option<u16>, ActionError> {
        self.inner.playback_at(page, slot)
    }

    fn group_playback(
        &self,
        _context: &ActionContext,
        _group_id: PlaybackGroupId,
    ) -> Result<Option<u16>, ActionError> {
        Ok(self.playback)
    }

    fn execute(
        &self,
        context: &ActionContext,
        address: ResolvedPlaybackAddress,
        action: PlaybackAction,
        surface: PlaybackSurface,
    ) -> Result<PlaybackExecution, ActionError> {
        self.inner.execute(context, address, action, surface)
    }

    fn projection(
        &self,
        context: &ActionContext,
        identity: PlaybackRuntimeIdentity,
    ) -> Result<PlaybackRuntimeProjection, ActionError> {
        self.inner.projection(context, identity)
    }

    fn desk_projection(
        &self,
        context: &ActionContext,
    ) -> Result<Option<PlaybackDeskProjection>, ActionError> {
        self.inner.desk_projection(context)
    }
}

#[test]
fn master_resolves_assignment_and_retains_group_runtime_identity() {
    let service = PlaybackService::default();
    let ports = GroupPorts::new(Some(8));
    let group_id = PlaybackGroupId::new("front").unwrap();

    let result = service
        .handle(
            envelope_with_action(
                ActionSource::Http,
                PlaybackAddress::Group(group_id.clone()),
                PlaybackAction::Master(PlaybackLevel::new(0.5)),
                None,
            ),
            &ports,
        )
        .unwrap();

    assert_eq!(
        result.resolved,
        ResolvedPlaybackAddress::Group {
            group_id: group_id.clone(),
            playback_number: Some(8),
        }
    );
    assert_eq!(
        ports.projection_reads(),
        vec![
            PlaybackRuntimeIdentity::Group(group_id.clone()),
            PlaybackRuntimeIdentity::Group(group_id),
        ]
    );
    assert_eq!(ports.actions()[0].address, result.resolved);
}

#[test]
fn unsupported_action_is_rejected_before_port_reads() {
    let service = PlaybackService::default();
    let ports = GroupPorts::new(None);
    let group_id = PlaybackGroupId::new("front").unwrap();

    let error = service
        .handle(
            envelope(ActionSource::Http, PlaybackAddress::Group(group_id), None),
            &ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(ports.actions().is_empty());
    assert!(ports.projection_reads().is_empty());
}
