// @bench-semantic-world

import { expect } from "@playwright/test";
import { fixtureRange } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { fixture } from "./bench/output/fixtureDmx";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
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

		await t.crossSurface.executeOscGroupCommandAndVerifyOutput();
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

		await t.crossSurface.verifyCurrentAndExplicitPageOscAddressing();
		await t.playback.expect(second).runtime({ enabled: true });
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
	"authenticated revisioned membership updates preserve one atomic visible Group result",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		const before = await t.group.snapshot(3);
		expect(before).not.toBeNull();
		if (!before) throw new Error("Expected canonical Group 3");
		await t.group.expectApiAuthenticationRejected(3, before.revision);
		await t.selection.fixtures.via.api.item(5);
		const changed = await t.group.recordViaApi(
			3,
			StoreMode.Merge,
			before.revision,
			"api-001-merge",
		);
		expect(changed).toMatchObject({
			status: "changed",
			replayed: false,
			group: { state: "stored", id: "3", revision: before.revision + 1 },
		});
		const afterChanged = await t.group.snapshot(3);
		await t.group.expectStaleApiRecordRejected(
			3,
			StoreMode.Merge,
			before.revision,
			before.revision + 1,
		);
		expect(await t.group.snapshot(3)).toEqual(afterChanged);
		await t.group.expect(3).fixtures(1, 2, 3, 4, 5);
		const after = await t.group.snapshot(3);
		expect(after?.revision).toBe(before.revision + 1);
	},
);

scenario(
	"API-002",
	"Group and Cuelist/Cue typed mutations remain ordered visible operations",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.items(1, 2);
		const created = await t.group.recordViaApi(
			90,
			StoreMode.Overwrite,
			0,
			"api-002-create",
		);
		await t.selection.fixtures.via.api.item(3);
		const merged = await t.group.recordViaApi(
			90,
			StoreMode.Merge,
			created.group.revision,
			"api-002-merge",
		);
		await t.group.expect(90).fixtures(1, 2, 3);
		const deleted = await t.group.recordViaApi(
			90,
			"delete",
			merged.group.revision,
			"api-002-delete",
		);
		expect([
			created.group.revision,
			merged.group.revision,
			deleted.group.revision,
		]).toEqual([1, 2, 3]);
		const eventSequences = [
			created.status === "changed" ? created.eventSequence : null,
			merged.status === "changed" ? merged.eventSequence : null,
			deleted.status === "changed" ? deleted.eventSequence : null,
		];
		expect(eventSequences).toEqual([
			expect.any(Number),
			expect.any(Number),
			expect.any(Number),
		]);
		expect(eventSequences[0]).toBeLessThan(eventSequences[1] as number);
		expect(eventSequences[1]).toBeLessThan(eventSequences[2] as number);
		expect(deleted.group).toMatchObject({ state: "deleted", id: "90" });
		await t.group.expect(90).absent();

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const cueCreated = await t.record.recordViaApi({ playback: 1, cue: 90 });
		expect(cueCreated).toMatchObject({
			status: "changed",
			recordedCue: { number: 90, deleted: false },
		});
		await t.selection.fixtures.via.api.item(2);
		await t.encoder.intensity.dimmer.via.api.set(75);
		const cueOverwritten = await t.record.recordViaApi({
			playback: 1,
			cue: 90,
		});
		expect(cueOverwritten).toMatchObject({
			status: "changed",
			recordedCue: { number: 90, deleted: false },
		});
		if (cueCreated.status !== "changed" || cueOverwritten.status !== "changed")
			throw new Error("Cue create and overwrite must both produce events");
		expect(cueCreated.showEventSequence).toBeLessThan(
			cueOverwritten.showEventSequence,
		);
		await t.cue.expect(1, 90).present();
		const siblingCue = await t.record.recordViaApi({ playback: 1, cue: 91 });
		if (siblingCue.status !== "changed")
			throw new Error("Second Cue must produce an event before deletion");
		expect(cueOverwritten.showEventSequence).toBeLessThan(
			siblingCue.showEventSequence,
		);
		await t.cue.expect(1, 91).present();
		const cueDeleted = await t.cue.deleteViaApi(1, 90);
		expect(cueDeleted).toMatchObject({
			status: "changed",
			deletedCue: { number: 90 },
		});
		if (cueDeleted.status !== "changed")
			throw new Error("Cue deletion must produce an event");
		expect(siblingCue.showEventSequence).toBeLessThan(
			cueDeleted.showEventSequence,
		);
		await t.cue.expect(1, 90).absent();
		await t.cue.expect(1, 91).present();
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
	"CROSS-001",
	"typed API Group value produces the shared Programmer and wire output",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.applyGroupOneAtFiftyViaApi();
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
	},
);

scenario(
	"CROSS-001",
	"real OSC Group command produces the shared Programmer and wire output",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.applyGroupOneAtFiftyViaOsc();
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
	},
);

scenario(
	"CROSS-002",
	"browser live-reconciles external REST and typed live-action mutations",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.crossSurface.reconcileExternalGroupMutation();
		await t.group.expect(3).fixtures(1, 2, 3, 4, 5);
	},
);
