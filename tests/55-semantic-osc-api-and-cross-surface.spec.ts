// @bench-semantic-world

import { fixtureRange } from "./bench/command-selection/selectionContract";
import { fixture } from "./bench/output/fixtureDmx";
import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import {
	currentPagePlayback,
	PlaybackButton,
} from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"OSC-001",
	"page changes produce one complete feedback cycle without periodic mutation",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.page.create(2);
		await t.page.rename(2, "Page 2");

		await t.crossSurface.expectCompleteFeedbackForPage("Page 2");
		await t.page.expect(2).selected();
	},
);

scenario(
	"OSC-002",
	"hardware-equivalent command reaches shared programmer and output",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("GROUP 1 AT 25");
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 64 });
	},
);

scenario(
	"OSC-003",
	"separate desk subscribers isolate partial commands and unsubscribe independently",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.expectDeskSubscriberIsolation();
	},
);

scenario(
	"OSC-004",
	"invalid input is rejected without programmer or output mutation",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.rejectInvalidGroupCommand();
	},
);

scenario(
	"OSC-005",
	"completed values are user-shared while unfinished commands stay desk-local",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.completeSharedValueWhilePeerDraftStaysLocal();
	},
);

scenario(
	"OSC-006",
	"page two retargets the same current-page playback-one action",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const first = await t.record.playback(1);
		await t.playback.via.api.off(first);
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(2);
		await t.encoder.intensity.dimmer.via.api.set(75);
		const second = await t.record.playback(2);
		await t.playback.via.api.off(second);
		await t.encoder.clear();
		await t.playback.configure(first, {
			buttons: [PlaybackButton.Go, PlaybackButton.GoBack, PlaybackButton.Flash],
		});
		await t.playback.configure(second, {
			buttons: [PlaybackButton.Go, PlaybackButton.GoBack, PlaybackButton.Flash],
		});
		await t.page.create(2);
		await t.page.rename(2, "Page 2");
		await t.page.map({ page: 2, slot: 1, playback: second });

		await t.page.via.ui.select(2);
		await t.playback.go(currentPagePlayback(1));
		await t.playback.expect(second).runtime({ current_cue_number: 1 });
		await t.playback.expect(first).runtime({ enabled: false });
		await t.playback.via.api.on(second);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 1), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(2, 2), { Intensity: 191 });
	},
);

scenario(
	"CROSS-003",
	"relative software, API, and OSC encoder changes bypass Programmer Fade",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("5s");

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.ui.set(0);
		await t.encoder.intensity.dimmer.via.ui.drag("add", "slow");
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 1 });
		await t.encoder.intensity.dimmer.via.ui.set(0);
		await t.encoder.intensity.dimmer.via.ui.add(10);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 26 });

		await t.hardware.connect();
		try {
			await t.encoder.intensity.dimmer.via.osc.add(1);
			await t.clock.advanceStep();
			await t.expectFixtureDMX(fixture(1), { Intensity: 28 });
		} finally {
			await t.hardware.disconnect();
		}

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(20);
		await t.selection.fixtures.via.api.item(2);
		await t.encoder.intensity.dimmer.via.api.set(80);
		await t.selection.fixtures.via.api.items(1, 2);
		await t.encoder.intensity.dimmer.via.api.add(10);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 77 });
		await t.expectFixtureDMX(fixture(2), { Intensity: 230 });
	},
);

scenario(
	"API-001",
	"authenticated membership updates preserve the visible atomic Group result",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.item(5);
		await t.group.via.keypad.store(3, { mode: StoreMode.Merge });
		await t.group.expect(3).fixtures(1, 2, 3, 4, 5);
	},
);

scenario(
	"API-002",
	"Group create, merge, and delete remain ordered visible operations",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.items(1, 2);
		await t.group.via.keypad.store(90, { mode: StoreMode.Overwrite });
		await t.selection.fixtures.via.api.item(3);
		await t.group.via.keypad.store(90, { mode: StoreMode.Merge });
		await t.group.expect(90).fixtures(1, 2, 3);
		await t.group.via.keypad.delete(90);
		await t.group.expect(90).absent();
	},
);

scenario(
	"CROSS-001",
	"equivalent group value agrees across command surfaces",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("GROUP 1 AT 50");
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
	},
);

scenario(
	"CROSS-002",
	"browser live-reconciles external REST and command-WebSocket mutations",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.reconcileExternalGroupMutation();
		await t.group.expect(3).fixtures(1, 2, 3, 4, 5);
	},
);
