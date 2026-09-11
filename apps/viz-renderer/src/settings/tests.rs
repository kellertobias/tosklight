use super::*;

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn the_default_connection_is_the_local_desk() {
    let options = Options::from_arguments(arguments(&[])).unwrap();
    assert_eq!(options.host, "127.0.0.1");
    assert_eq!(options.port, 5000);
    assert_eq!(options.source, ProviderKind::LightingDesk);
}

#[test]
fn a_remote_desk_address_is_accepted() {
    let options =
        Options::from_arguments(arguments(&["--server", "10.0.0.9", "--port", "5100"])).unwrap();
    assert_eq!(options.host, "10.0.0.9");
    assert_eq!(options.port, 5100);
}

#[test]
fn an_existing_planning_server_is_used_without_opening_another_editor() {
    let options = Options::from_arguments(arguments(&[
        "--planning-server",
        "127.0.0.1",
        "--port",
        "5311",
    ]))
    .unwrap();
    assert_eq!(options.host, "127.0.0.1");
    assert_eq!(options.port, 5311);
    assert_eq!(options.source, ProviderKind::PlanningSoftware);
    assert!(options.planning_server_requested);
    assert_eq!(options.startup(), Startup::Desk);

    let mut preferences = Preferences::from_options(&options);
    preferences.adopt_file("source lighting_desk\nhost old-desk\nport 5000\n", &options);
    assert_eq!(preferences.source, ProviderKind::PlanningSoftware);
    assert_eq!(preferences.host, "127.0.0.1");
    assert_eq!(preferences.port, 5311);
}

#[test]
fn ports_outside_the_valid_range_are_rejected_with_a_readable_message() {
    assert!(parse_port("0").is_err());
    assert!(parse_port("65536").is_err());
    assert!(parse_port("nope").is_err());
    assert_eq!(parse_port(" 5000 "), Ok(5000));
}

#[test]
fn staged_edits_validate_before_they_can_replace_the_live_connection() {
    let preferences = Preferences::from_options(&Options::default());
    let mut staged = StagedConnection::from_preferences(&preferences);
    staged.port_text = "70000".into();
    assert!(staged.validate().is_err());
    staged.port_text = "5001".into();
    staged.host = "  desk.local ".into();
    assert_eq!(staged.validate(), Ok(("desk.local".to_owned(), 5001)));
}

#[test]
fn a_bare_launch_opens_the_planning_window() {
    // Nobody named a console and nobody named a file: there is nothing to draw and no way to
    // say what to draw, so the operator gets somewhere to choose.
    let options = Options::from_arguments(arguments(&[])).unwrap();
    assert_eq!(
        options.startup_with_recent_show(None, false),
        Startup::Planning
    );
}

#[test]
fn a_named_desk_is_visualized_directly() {
    let options = Options::from_arguments(arguments(&["--server", "10.0.0.9"])).unwrap();
    assert_eq!(options.startup(), Startup::Desk);
    let options = Options::from_arguments(arguments(&["--port", "5001"])).unwrap();
    assert_eq!(
        options.startup(),
        Startup::Desk,
        "naming only the port still names a desk"
    );
}

#[test]
fn a_show_file_is_served_and_visualized_without_a_planning_window() {
    let options = Options::from_arguments(arguments(&["--show", "/shows/tour.show"])).unwrap();
    assert_eq!(
        options.startup(),
        Startup::Show(PathBuf::from("/shows/tour.show"))
    );
}

#[test]
fn a_named_desk_wins_over_an_absent_one() {
    // The desk launches the visualizer with its own address; that must never be mistaken for
    // a bare launch and turned into a planning window on an operator's show surface.
    let options =
        Options::from_arguments(arguments(&["--server", "127.0.0.1", "--port", "5000"])).unwrap();
    assert_eq!(options.startup(), Startup::Desk);
}

#[test]
fn the_demo_scene_overrides_every_other_source() {
    let options =
        Options::from_arguments(arguments(&["--demo", "--show", "/shows/tour.show"])).unwrap();
    assert_eq!(options.startup(), Startup::Demo);
}

#[test]
fn quality_defaults_to_following_the_source() {
    let preferences = Preferences::from_options(&Options::default());
    assert_eq!(preferences.quality_label(), "Follow source");
}
