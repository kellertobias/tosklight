use super::temporary;
use crate::{
    DeskStore, PlaybackControlPresentation, PlaybackPageSource, PlaybackSurfaceConfiguration,
    PlaybackSurfaceLayout, PlaybackSurfaceRow, resolve_playback_surface_pages,
};
use std::fs;
use uuid::Uuid;

fn surface(
    first_position: u16,
    count: u16,
    page_source: PlaybackPageSource,
) -> PlaybackSurfaceConfiguration {
    PlaybackSurfaceConfiguration {
        name: "Test".into(),
        page_source,
        layout: PlaybackSurfaceLayout {
            rows: vec![PlaybackSurfaceRow {
                first_position,
                count,
                presentation: PlaybackControlPresentation::Buttons,
                button_count: 1,
            }],
        },
    }
}

#[test]
fn exact_surface_layout_and_selected_page_persist_without_allocating_playback_positions() {
    let path = temporary("unified-surfaces");
    let desk = DeskStore::open(&path).unwrap();
    let desk_id = Uuid::new_v4();
    let mut surfaces = desk.playback_surfaces(desk_id).unwrap();
    surfaces.insert(
        "screen:second".into(),
        surface(201, 50, PlaybackPageSource::Own { page: 99 }),
    );
    surfaces.insert(
        "pane:wide".into(),
        surface(
            401,
            280,
            PlaybackPageSource::Follow {
                surface_id: "screen:second".into(),
            },
        ),
    );
    desk.put_playback_surfaces(desk_id, &surfaces).unwrap();
    desk.select_playback_surface(desk_id, "pane:wide").unwrap();
    drop(desk);
    let reopened = DeskStore::open(&path).unwrap();
    assert_eq!(reopened.playback_surfaces(desk_id).unwrap(), surfaces);
    assert_eq!(
        reopened.selected_playback_surface(desk_id).unwrap(),
        "pane:wide"
    );
    let pages = resolve_playback_surface_pages(&surfaces).unwrap();
    assert_eq!(pages["pane:wide"], 99);
    assert_eq!(surfaces["main"].layout.rows[0].count, 15);
    assert_eq!(surfaces["main"].layout.rows[1].first_position, 101);
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn cyclic_dangling_and_out_of_range_edits_leave_surface_registry_unchanged() {
    let path = temporary("invalid-surfaces");
    let desk = DeskStore::open(&path).unwrap();
    let desk_id = Uuid::new_v4();
    let original = desk.playback_surfaces(desk_id).unwrap();
    for invalid in [
        surface(1000, 2, PlaybackPageSource::Own { page: 1 }),
        surface(401, 280, PlaybackPageSource::Own { page: 100 }),
        surface(
            1,
            1,
            PlaybackPageSource::Follow {
                surface_id: "missing".into(),
            },
        ),
        surface(
            1,
            1,
            PlaybackPageSource::Follow {
                surface_id: "bad".into(),
            },
        ),
    ] {
        let mut candidate = original.clone();
        candidate.insert("bad".into(), invalid);
        assert!(desk.put_playback_surfaces(desk_id, &candidate).is_err());
        assert_eq!(desk.playback_surfaces(desk_id).unwrap(), original);
    }
    assert!(desk.select_playback_surface(desk_id, "missing").is_err());
    assert_eq!(desk.selected_playback_surface(desk_id).unwrap(), "main");
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn different_desks_keep_separate_pages_layouts_and_selection() {
    let path = temporary("surface-isolation");
    let store = DeskStore::open(&path).unwrap();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let mut registry = store.playback_surfaces(a).unwrap();
    registry.get_mut("main").unwrap().page_source = PlaybackPageSource::Own { page: 37 };
    registry.insert(
        "screen:a".into(),
        surface(201, 50, PlaybackPageSource::Own { page: 8 }),
    );
    store.put_playback_surfaces(a, &registry).unwrap();
    store.select_playback_surface(a, "screen:a").unwrap();
    assert_eq!(
        store.playback_surfaces(b).unwrap()["main"].page_source,
        PlaybackPageSource::Own { page: 1 }
    );
    assert_eq!(store.selected_playback_surface(b).unwrap(), "main");
    assert!(!store.playback_surfaces(b).unwrap().contains_key("screen:a"));
    assert_eq!(store.selected_playback_surface(a).unwrap(), "screen:a");
    drop(store);
    let _ = fs::remove_file(path);
}
