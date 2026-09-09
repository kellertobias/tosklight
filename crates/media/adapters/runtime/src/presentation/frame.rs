use super::*;

pub(super) fn operator_overlay<'a>(
    visible_until: Option<std::time::Instant>,
    source: Option<&'a SourceTexture>,
    layer: &'a media_domain::LayerState,
) -> Option<LayerDraw<'a>> {
    visible_until
        .filter(|until| *until > std::time::Instant::now())
        .and(source)
        .map(|source| LayerDraw {
            state: layer,
            source,
            mask: None,
        })
}

pub(super) fn present_standby(
    sinks: &mut CaptureSinks,
    test_pattern_layer: &media_domain::LayerState,
    operator_overlay_layer: &media_domain::LayerState,
    hosted: &mut HostedOutput,
    output_state: &media_domain::OutputState,
    status_overlay: bool,
    now: Timestamp,
    region: Option<&media_domain::display_region::DisplayRegion>,
) -> bool {
    if !crate::standby::visible(
        status_overlay,
        output_state.ownership.dmx.is_some(),
        output_state.ownership.web_takeover,
    ) {
        return false;
    }
    let Some(standby) = hosted.standby.as_ref() else {
        return false;
    };
    let draws = [LayerDraw {
        state: test_pattern_layer,
        source: standby,
        mask: None,
    }];
    let idle = MasterState::default();
    let overlay = operator_overlay(
        hosted.hint_visible_until,
        hosted.fullscreen_hint.as_ref(),
        operator_overlay_layer,
    );
    present(
        &mut hosted.output,
        &draws,
        &idle,
        None,
        now,
        region,
        overlay,
    );
    capture_previews(
        sinks,
        &hosted.configuration,
        &mut hosted.output,
        output_state,
        &[],
        &draws,
        &MasterState::default(),
        None,
        now,
    );
    true
}

/// Presents the development clip, when one was named at launch.
///
/// Its presence consumes this output pass even while the loader is still preparing the frame,
/// matching the launch affordance's precedence over the live library pipeline.
#[allow(clippy::too_many_arguments)]
pub(super) fn present_direct(
    loader: &mut AsyncClipLoader,
    direct: Option<&mut DirectClip>,
    sinks: &mut CaptureSinks,
    operator_overlay_layer: &media_domain::LayerState,
    hosted: &mut HostedOutput,
    output_state: &media_domain::OutputState,
    master: &MasterState,
    now: Timestamp,
    region: Option<&media_domain::display_region::DisplayRegion>,
) -> bool {
    let Some(direct) = direct else {
        return false;
    };
    if !matches!(loader.request_load(direct.asset, &direct.path), Ok(Some(_))) {
        return true;
    }
    let delivery = direct
        .session
        .deliver(&direct.layer, media_domain::ResolvedTempo::None, now);
    if let Some(frame) = delivery.frame
        && hosted
            .sources
            .prepare(0, direct.asset, frame, direct.size, loader)
            .unwrap_or(false)
        && let Some(texture) = hosted.sources.texture(0)
    {
        let draws = [LayerDraw {
            state: &direct.layer,
            source: texture,
            mask: None,
        }];
        let overlay = operator_overlay(
            hosted.hint_visible_until,
            hosted.fullscreen_hint.as_ref(),
            operator_overlay_layer,
        );
        present(
            &mut hosted.output,
            &draws,
            master,
            None,
            now,
            region,
            overlay,
        );
        capture_previews(
            sinks,
            &hosted.configuration,
            &mut hosted.output,
            output_state,
            std::slice::from_ref(&direct.layer),
            &draws,
            master,
            None,
            now,
        );
    }
    true
}
