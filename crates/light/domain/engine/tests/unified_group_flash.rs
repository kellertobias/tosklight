use light_engine::Engine;
use light_playback::PlaybackControlSource;
use light_programmer::ProgrammerRegistry;
#[test]
fn releasing_and_cancelling_one_surface_preserves_other_group_flash_holds() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let a = PlaybackControlSource {
        surface_id: "main".into(),
        control_id: 1,
    };
    let b = PlaybackControlSource {
        surface_id: "hardware:other".into(),
        control_id: 1,
    };
    engine.set_group_master_flash_source("front".into(), &a, 1.0);
    engine.set_group_master_flash_source("front".into(), &b, 0.8);
    engine.set_group_master_flash_source("front".into(), &a, 0.0);
    assert_eq!(engine.group_master_flash("front"), 0.8);
    engine.set_group_master_flash_source("front".into(), &a, 1.0);
    engine.cancel_group_flash_surface("main");
    assert_eq!(engine.group_master_flash("front"), 0.8);
    engine.cancel_group_flash_surface("hardware:other");
    assert_eq!(engine.group_master_flash("front"), 0.0);
}
