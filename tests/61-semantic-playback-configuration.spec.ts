// @bench-semantic-world

import { fixture } from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { StoreMode as GroupStoreMode } from "../apps/control-ui/e2e/bench/groups-presets/groupScenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"PBK-001",
	"Set inspection resolves one playback identity and Close is mutation-free",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.playbackConfiguration.prepareInspection();
		await t.app.open();
		await t.app.expect.ready();
		await t.playbackConfiguration.expectInspectionWithoutMutation(1, {
			number: 40,
			targetType: "cue_list",
			buttons: ["go_minus", "go", "flash"],
			buttonCount: 3,
			fader: "master",
			hasFader: true,
			color: "#20c997",
		});
	},
);

scenario(
	"PBK-002",
	"Cue List assignment, color, and None plus Apply clear are atomic",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.playbackConfiguration.prepareAssignment();
		await t.app.open();
		await t.app.expect.ready();
		await t.playbackConfiguration.assignCueList(
			1,
			"Configured Sequence",
			"#8b5cf6",
		);
		await t.playbackConfiguration.expectAssignment(1, {
			targetType: "cue_list",
			cueList: "Configured Sequence",
			buttons: ["go_minus", "go", "flash"],
			buttonCount: 3,
			fader: "master",
			hasFader: true,
			color: "#8b5cf6",
		});
		await t.playbackConfiguration.clear(1);
		await t.playbackConfiguration.expectUnassigned(1);
		await t.playbackConfiguration.expectCueListPresent("Configured Sequence");
	},
);

scenario(
	"PBK-003",
	"default navigation and remapped Select Contents dispatch one exact action",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.playbackConfiguration.prepareActionMatrix();
		await t.app.open();
		await t.app.expect.ready();
		await t.playback.go(43);
		await t.playback.go(43);
		await t.playback.goBack(43);
		await t.playback.expect(43).runtime({ current_cue_number: 1 });
		await t.playbackConfiguration.setButton(1, "Top button", "Select contents");
		await t.playbackConfiguration.expectAssignment(1, {
			number: 43,
			buttons: ["select_contents", "go", "flash"],
		});
		await t.playbackConfiguration.selectContentsWithoutPlaybackMutation(1, 43, {
			fixtures: [1, 2],
			group: 3,
		});
	},
);

scenario(
	"PBK-004",
	"X-fade travel advances one Cue and preserves manual direction and timing",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.playbackConfiguration.prepareCrossfade();
		await t.app.open();
		await t.app.expect.ready();
		await t.playback.fader(47, 25);
		await t.clock.advanceBy("0ms");
		await t.playback.expect(47).runtime({
			current_cue_number: 1,
			manual_xfade_position: 0.25,
			manual_xfade_progress: 0.25,
			manual_xfade_direction: "towards_high",
		});
		await t.expectFixtureValue(fixture(1), { intensity: 0.25 });
		await t.playback.fader(47, 100);
		await t.clock.advanceBy("0ms");
		await t.playback.expect(47).runtime({
			current_cue_number: 2,
			manual_xfade_position: 1,
			manual_xfade_direction: "towards_low",
		});
		await t.expectFixtureValue(fixture(1), { intensity: 1 });
		await t.cue
			.expect(47, 1)
			.metadata({ fade_millis: 30_000, delay_millis: 10_000 });
		await t.cue
			.expect(47, 2)
			.metadata({ fade_millis: 30_000, delay_millis: 10_000 });
	},
);

scenario(
	"PBK-005",
	"Temp and held Swap have explicit lifetimes and restore the underlying playback",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.playbackConfiguration.prepareTempAndSwap();
		await t.app.open();
		await t.app.expect.ready();
		await t.playback.open();
		await t.playback.via.ui.temp(55);
		await t.playbackConfiguration.expectTemporary(55, true);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureValue(fixture(1), { intensity: 0.8 });
		await t.expectFixtureValue(fixture(2), { intensity: 0.6 });
		await t.expectFixtureValue(fixture(3), { intensity: 0.4 });
		await t.playback.via.ui.temp(55);
		await t.playbackConfiguration.expectTemporary(55, false);
		await t.expectFixtureValue(fixture(1), { intensity: 0.3 });

		await t.playback.via.ui.swap(55).hold(async () => {
			await t.playbackConfiguration.expectSwap(55, true);
			await t.clock.advanceBy("0ms");
			await t.expectFixtureValue(fixture(1), { intensity: 0.8 });
			await t.expectFixtureValue(fixture(2), { intensity: 0 });
			await t.expectFixtureValue(fixture(3), { intensity: 0.4 });
		});
		await t.playbackConfiguration.expectSwap(55, false);
		await t.expectFixtureValue(fixture(1), { intensity: 0.3 });
		await t.expectFixtureValue(fixture(2), { intensity: 0.6 });
		await t.expectFixtureValue(fixture(3), { intensity: 0.4 });
		await t.playback.expect(54).runtime({ enabled: true });
		await t.playback.expect(56).runtime({ enabled: true });
		await t.playback.expect(57).runtime({ enabled: true });
	},
);

scenario(
	"PBK-006",
	"specialized layouts control their authoritative Speed, Group, Grand, and Fade masters",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.playbackConfiguration.prepareMasters();
		await t.app.open();
		await t.app.expect.ready();
		await t.playbackConfiguration.press(1, "DOUBLE");
		await t.playbackConfiguration.setSlotFader(1, 0.5);
		await t.playbackConfiguration.setSlotFader(2, 0.4);
		await t.playbackConfiguration.press(2, "SELECT");
		await t.playbackConfiguration.setSlotFader(3, 0.3);
		await t.playbackConfiguration.press(3, "BLACKOUT");
		await t.playbackConfiguration.setSlotFader(4, 0.25);
		await t.playbackConfiguration.setSlotFader(5, 0.25);
		await t.playbackConfiguration.expectMasters({
			speed: { manualBpm: 240, effectiveBpm: 120, paused: false },
			neighborBpms: [96, 72, 60, 48],
			group: { master: 0.4, flashLevel: 0 },
			grand: {
				level: 0.3,
				effectiveLevel: 0.3,
				blackout: true,
				dynamicsPaused: false,
			},
			programmerFadeMillis: 5_000,
			cueFadeMillis: 15_000,
		});
		await t.playbackConfiguration.expectAssignment(1, {
			number: 61,
			targetType: "speed_group",
			buttons: ["double", "half", "learn"],
			fader: "learned_percentage",
			color: "#8b5cf6",
		});
		await t.playbackConfiguration.expectAssignment(2, {
			number: 62,
			targetType: "group",
		});
		await t.playbackConfiguration.expectAssignment(3, {
			number: 63,
			targetType: "grand_master",
		});
	},
);

scenario(
	"PBK-006",
	"independent overlapping Group Masters use the highest assigned level",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.playbackConfiguration.clearPage();
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.items(1, 2, 3, 4, 5, 6);
		await t.group.via.api.store(5, { mode: GroupStoreMode.Overwrite });
		await t.selection.fixtures.via.api.items(1, 3, 5);
		await t.group.via.api.store(6, { mode: GroupStoreMode.Overwrite });
		await t.selection.fixtures.via.api.items(2, 4, 6);
		await t.group.via.api.store(7, { mode: GroupStoreMode.Overwrite });
		await t.selection.clear();

		await t.playbackConfiguration.assignGroupMaster(1, 6);
		await t.playbackConfiguration.assignGroupMaster(2, 7);
		await t.command.execute("GROUP 7 AT 100");
		await t.playbackConfiguration.setSlotFader(2, 0.8);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
		await t.expectFixtureDMX(fixture(2), { Intensity: 204 });
		await t.expectFixtureDMX(fixture(4), { Intensity: 204 });
		await t.expectFixtureDMX(fixture(6), { Intensity: 204 });

		await t.playbackConfiguration.assignGroupMaster(3, 5);
		await t.playbackConfiguration.setSlotFader(3, 0.6);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(2), { Intensity: 204 });
		await t.expectFixtureDMX(fixture(4), { Intensity: 204 });
		await t.expectFixtureDMX(fixture(6), { Intensity: 204 });

		await t.playbackConfiguration.assignGrandMaster(4);
		await t.playbackConfiguration.setSlotFader(4, 0.5);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(2), { Intensity: 102 });
		await t.expectFixtureDMX(fixture(4), { Intensity: 102 });
		await t.expectFixtureDMX(fixture(6), { Intensity: 102 });
	},
);
