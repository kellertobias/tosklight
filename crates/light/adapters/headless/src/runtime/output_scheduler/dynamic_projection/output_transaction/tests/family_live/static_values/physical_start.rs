//! Physical ON must retain the accepted Dynamic color, rather than a hidden static peer.
use super::*;
use light_engine::{EnginePlaybackCommand, PoolPlaybackAction};
use light_playback::{Cue, CueChange, CueList, PlaybackDefinition};

#[test]
fn physical_on_captures_real_dynamic_color_from_published_family_output() {
    let target = FixtureId::new();
    let desk = Desk::new(vec![patched(&rgb(), target, 1)]);
    let colors = [
        program(&intent([1., 1., 1.], 0.)),
        program(&intent([1., 0., 0.], 0.)),
    ];
    let lists: Vec<CueList> = colors.into_iter().map(|value| {
        let mut cue = Cue::new(1_u16.into());
        cue.changes.push(CueChange::set(target, ProgrammingOwner::Color.key(), value));
        serde_json::from_value(serde_json::json!({"id":light_core::CueListId::new(),"name":"Published start","priority":10,"mode":"sequence","looped":false,"cues":[cue]})).unwrap()
    }).collect();
    let definitions: Vec<PlaybackDefinition> = lists.iter().enumerate().map(|(index,list)| {
        serde_json::from_value(serde_json::json!({"number":index+1,"name":"Physical","target":{"type":"cue_list","cue_list_id":list.id},"auto_off":false})).unwrap()
    }).collect();
    let mut snapshot = (*desk.state.output.snapshot()).clone();
    snapshot.cue_lists = lists.into();
    snapshot.playbacks = definitions.into();
    snapshot.revision += 1;
    desk.state.output.replace_snapshot(snapshot).unwrap();
    let on = |number| {
        desk.state
            .output
            .execute_playback(EnginePlaybackCommand::Pool {
                number,
                action: PoolPlaybackAction::On,
            })
            .unwrap()
    };
    on(1);
    desk.clock.advance_millis(3000);
    desk.state
        .output
        .set_control_timing([120.; 5], 0, 2000, 2000);
    fix(
        &desk.programmers,
        &desk.clock,
        desk.session,
        target,
        ProgrammingOwner::Color,
        program(&intent([1., 1., 1.], 0.)),
        program(&intent([0., 1., 0.], 0.)),
    );
    let (_, published) = desk.frame();
    let initial = writes(&published, ProgrammingOwner::Color, target);
    assert!(
        initial.iter().any(|(_, raw)| *raw > 200),
        "Dynamic must emit before activation: {initial:?}"
    );
    on(2);
    desk.programmers.clear(desk.session);
    desk.clock.advance_millis(975);
    let (_, published) = desk.frame();
    let crossing = writes(&published, ProgrammingOwner::Color, target);
    // RGB's channel slots are ordered R,G,B. Both red and green must contribute in the fade;
    // neither the hidden white base (blue) nor a held green endpoint is the accepted crossing.
    let rgb: Vec<_> = crossing.iter().map(|(_, raw)| *raw).collect();
    assert_eq!(rgb.len(), 3, "{crossing:?}");
    assert!(
        rgb[0] > 30 && rgb[1] > 30 && rgb[2] < 5,
        "accepted Dynamic green must fade toward red: {rgb:?}"
    );
}
