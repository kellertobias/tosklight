import { describe, expect, it, vi } from "vitest";
import type { PlaybackSnapshot } from "../../api/types";
import type { ServerController } from "./model";
import { createGroupEditingActions } from "./groupEditing";
import { ShowObjectsStore } from "../showObjects/store";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

function harness(
	write: Promise<{ revision: number; event_sequence: number | null }>,
) {
	const showObjectsStore = new ShowObjectsStore();
	showObjectsStore.reset(SHOW_ID);
	showObjectsStore.setCollection(SHOW_ID, "group", [
		{
			kind: "group",
			id: "1",
			revision: 3,
			updated_at: "",
			body: { name: "Front", fixtures: ["fixture-1"], master: 0.2 },
		},
	]);
	const client = {
		putObject: vi.fn(
			(
				_showId: string,
				_kind: string,
				_id: string,
				_body: unknown,
				_revision: number,
			) => write,
		),
		setGroupMaster: vi.fn().mockResolvedValue({ group_id: "1", master: 0.7 }),
		objects: vi.fn(),
		bootstrap: vi.fn(),
		shows: vi.fn(),
		configuration: vi.fn(),
		mediaServers: vi.fn(),
		fixtureLibrary: vi.fn(),
		fixtureProfiles: vi.fn(),
		patch: vi.fn(),
	};
	const setError = vi.fn();
	let playbacks = {
		cue_lists: [],
		pool: [],
		pages: [],
		active: [],
		desk: {
			id: "desk",
			name: "Desk",
			osc_alias: "main",
			columns: 1,
			rows: 1,
			buttons: 1,
		},
		active_page: 1,
		authoritative_controls: {
			speed_groups: [],
			groups: [{ id: "1", master: 0.2, flash_level: 0 }],
			grand_master: {
				level: 1,
				blackout: false,
				flash_active: false,
				dynamics_paused: false,
			},
			programmer_fade_millis: 0,
			cue_fade_millis: 0,
		},
	} as PlaybackSnapshot;
	const setPlaybacks = vi.fn(
		(next: (current: PlaybackSnapshot | null) => PlaybackSnapshot | null) => {
			playbacks = next(playbacks) as PlaybackSnapshot;
		},
	);
	const model = {
		client,
		setError,
		bootstrap: { active_show: { id: SHOW_ID } },
		playbacks,
		showObjectsStore,
		setPlaybacks,
	} as unknown as ServerController;
	return {
		client,
		setError,
		showObjectsStore,
		get playbacks() {
			return playbacks;
		},
		actions: createGroupEditingActions(model),
	};
}

describe("Group optimistic object mutation", () => {
	it("updates immediately and commits without broad resource reloads", async () => {
		const test = harness(Promise.resolve({ revision: 4, event_sequence: 12 }));
		const result = test.actions.updateGroup("1", {
			name: "Front Wash",
			color: "#123456",
			icon: "◆",
		});

		expect(test.showObjectsStore.getSnapshot().groups[0].body.name).toBe(
			"Front Wash",
		);
		await expect(result).resolves.toBe(true);
		expect(test.showObjectsStore.getSnapshot().groups[0]).toMatchObject({
			revision: 4,
			body: { name: "Front Wash", color: "#123456", icon: "◆" },
		});
		expect(test.client.putObject).toHaveBeenCalledOnce();
		expect(test.client.putObject.mock.calls[0][3]).toMatchObject({
			name: "Front Wash",
			master: 0.2,
		});
		for (const request of [
			test.client.objects,
			test.client.bootstrap,
			test.client.shows,
			test.client.configuration,
			test.client.mediaServers,
			test.client.fixtureLibrary,
			test.client.fixtureProfiles,
			test.client.patch,
		])
			expect(request).not.toHaveBeenCalled();
	});

	it("rolls the optimistic object back when the write fails", async () => {
		const test = harness(Promise.reject(new Error("revision conflict")));
		const result = test.actions.updateGroup("1", {
			name: "Front Wash",
			color: "#123456",
			icon: "◆",
		});

		expect(test.showObjectsStore.getSnapshot().groups[0].body.name).toBe(
			"Front Wash",
		);
		await expect(result).resolves.toBe(false);
		expect(test.showObjectsStore.getSnapshot().groups[0].body.name).toBe("Front");
		expect(test.showObjectsStore.getSnapshot().pendingObjectKeys.size).toBe(0);
		expect(test.setError).toHaveBeenLastCalledWith("revision conflict");
	});

	it("reads the latest Group snapshot when an action is invoked", async () => {
		const test = harness(Promise.resolve({ revision: 9, event_sequence: 12 }));
		test.showObjectsStore.setCollection(SHOW_ID, "group", [
			{
				kind: "group",
				id: "1",
				revision: 8,
				updated_at: "",
				body: {
					name: "Newer snapshot",
					fixtures: ["fixture-1", "fixture-2"],
					master: 0.4,
				},
			},
		]);

		await test.actions.updateGroup("1", { name: "Renamed" });

		expect(test.client.putObject).toHaveBeenCalledWith(
			SHOW_ID,
			"group",
			"1",
			expect.objectContaining({
				name: "Renamed",
				fixtures: ["fixture-1", "fixture-2"],
				master: 0.4,
			}),
			8,
		);
	});

	it("keeps runtime Group master feedback out of portable Group state", async () => {
		const test = harness(Promise.resolve({ revision: 4, event_sequence: 12 }));

		await test.actions.setGroupMaster("1", 0.7);

		expect(test.showObjectsStore.getSnapshot().groups[0].body.master).toBe(0.2);
		expect(
			test.playbacks.authoritative_controls?.groups.find(
				(group) => group.id === "1",
			)?.master,
		).toBe(0.7);
	});
});
