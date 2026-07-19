import { describe, expect, it } from "vitest";
import type {
	CommandLineProjection,
	ProgrammingChange,
	ProgrammingSnapshot,
	SelectionProjection,
} from "./contracts";
import { selectedGroupId } from "./contracts";
import { ProgrammingInteractionStore } from "./store";
import { ProgrammingProtocolError } from "./transport";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const DESK_ID = "11111111-1111-4111-8111-111111111111";
const OTHER_DESK_ID = "99999999-9999-4999-8999-999999999999";
const FIXTURE_1 = "22222222-2222-4222-8222-222222222222";
const FIXTURE_2 = "33333333-3333-4333-8333-333333333333";
const FIXTURE_3 = "44444444-4444-4444-8444-444444444444";

function commandLine(
	revision = 1,
	text = "FIXTURE",
	target: CommandLineProjection["target"] = "FIXTURE",
): CommandLineProjection {
	return { text, target, pristine: text === target, revision, pendingChoice: null };
}

function selection(
	revision = 1,
	selected = [FIXTURE_1, FIXTURE_2],
	expression: SelectionProjection["expression"] = { type: "static" },
): SelectionProjection {
	return { selected, expression, revision, gestureOpen: false };
}

function snapshot(
	sequence = 10,
	command = commandLine(),
	selected = selection(),
): ProgrammingSnapshot {
	return {
		cursor: sequence,
		projection: {
			deskId: DESK_ID,
			commandLine: command,
			selection: selected,
		},
	};
}

function readyStore() {
	const store = new ProgrammingInteractionStore();
	store.reset(SHOW_ID, DESK_ID);
	store.installSnapshot(snapshot());
	return store;
}

describe("ProgrammingInteractionStore authority", () => {
	it("isolates state by show and desk", () => {
		const store = readyStore();
		store.reset("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", DESK_ID);

		expect(store.getSnapshot()).toMatchObject({
			showId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
			deskId: DESK_ID,
			commandLine: null,
			selection: null,
			status: "loading",
		});
		expect(store.installSnapshot(snapshot())).toBe(true);
		expect(
			store.applyChange(
				{
					deskId: "99999999-9999-4999-8999-999999999999",
					commandLine: commandLine(2),
				},
				12,
			),
		).toBe(false);
	});

	it("applies sparse capabilities and allows filtered sequence gaps", () => {
		const store = readyStore();
		store.applyChange(
			{ deskId: DESK_ID, commandLine: commandLine(2, "FIXTURE 7") },
			47,
		);

		expect(store.getSnapshot().commandLine?.text).toBe("FIXTURE 7");
		expect(store.getSnapshot().selection?.revision).toBe(1);
		expect(store.getSnapshot().eventSequence).toBe(47);
	});

	it("ignores stale component revisions without dropping a newer peer", () => {
		const store = readyStore();
		store.applyChange(
			{
				deskId: DESK_ID,
				commandLine: commandLine(3, "FIXTURE 3"),
				selection: selection(3, [FIXTURE_3]),
			},
			20,
		);
		store.applyChange(
			{
				deskId: DESK_ID,
				commandLine: commandLine(2, "STALE"),
				selection: selection(4, [FIXTURE_2, FIXTURE_1]),
			},
			24,
		);

		expect(store.getSnapshot().commandLine).toMatchObject({
			text: "FIXTURE 3",
			revision: 3,
		});
		expect(store.getSnapshot().selection).toMatchObject({
			selected: [FIXTURE_2, FIXTURE_1],
			revision: 4,
		});
	});

	it("rejects unequal projections at one revision atomically", () => {
		const store = readyStore();
		const conflicting: ProgrammingChange = {
			deskId: DESK_ID,
			commandLine: commandLine(2, "FIXTURE 2"),
			selection: selection(1, [FIXTURE_2, FIXTURE_1]),
		};

		expect(() => store.applyChange(conflicting, 31)).toThrowError(
			ProgrammingProtocolError,
		);
		expect(store.getSnapshot().commandLine?.revision).toBe(1);
		expect(store.getSnapshot().selection?.selected).toEqual([
			FIXTURE_1,
			FIXTURE_2,
		]);
	});

	it("derives the selected live Group from the selection expression", () => {
		expect(
			selectedGroupId(
				selection(1, [FIXTURE_1], {
					type: "live_group",
					groupId: "7",
					rule: { type: "odd" },
				}),
			),
		).toBe("7");
		expect(
			selectedGroupId(
				selection(1, [FIXTURE_1], {
					type: "sources",
					items: [{ type: "live_group", groupId: "8" }],
				}),
			),
		).toBe("8");
		expect(
			selectedGroupId(
				selection(1, [FIXTURE_1], {
					type: "frozen_group",
					groupId: "8",
					sourceRevision: 2,
				}),
			),
		).toBeNull();
	});
});

describe("ProgrammingInteractionStore optimism", () => {
	it("installs a command response before removing its optimistic operation", () => {
		const store = readyStore();
		const first = store.beginOptimisticCommandLine({ text: "FIXTURE 1" });
		const second = store.beginOptimisticCommandLine({ text: "FIXTURE 12" });

		expect(
			store.commitCommandLine(first, commandLine(2, "FIXTURE 1")),
		).toBe(true);
		expect(store.getSnapshot().commandLine).toMatchObject({
			text: "FIXTURE 12",
			revision: 2,
		});
		expect(store.getSnapshot().pendingCapabilities).toEqual(
			new Set(["commandLine"]),
		);

		expect(
			store.commitCommandLine(second, commandLine(3, "FIXTURE 12")),
		).toBe(true);
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(3, "FIXTURE 12"),
		);
		expect(store.getSnapshot().pendingCapabilities).toEqual(new Set());
	});

	it("accepts an authoritative event that arrives before the matching response", () => {
		const store = readyStore();
		const token = store.beginOptimisticCommandLine({ text: "FIXTURE 9" });
		const authoritative = commandLine(2, "FIXTURE 9");
		store.applyChange(
			{ deskId: DESK_ID, commandLine: authoritative },
			12,
		);

		expect(store.commitCommandLine(token, authoritative)).toBe(true);
		expect(store.getSnapshot().commandLine).toEqual(authoritative);
		expect(store.getSnapshot().eventSequence).toBe(12);
	});

	it("keeps a newer event when an older command response arrives", () => {
		const store = readyStore();
		const token = store.beginOptimisticCommandLine({ text: "FIXTURE 9" });
		const newest = commandLine(3, "GROUP 3", "GROUP");
		store.applyChange({ deskId: DESK_ID, commandLine: newest }, 13);

		expect(store.commitCommandLine(token, commandLine(2, "FIXTURE 9"))).toBe(
			true,
		);
		expect(store.getSnapshot().commandLine).toEqual(newest);
	});

	it("rejects a divergent response at the authoritative revision", () => {
		const store = readyStore();
		const token = store.beginOptimisticCommandLine({ text: "FIXTURE 9" });
		store.applyChange(
			{ deskId: DESK_ID, commandLine: commandLine(2, "GROUP 2", "GROUP") },
			12,
		);

		expect(() =>
			store.commitCommandLine(token, commandLine(2, "FIXTURE 9")),
		).toThrow(ProgrammingProtocolError);
		expect(store.rollback(token, new Error("conflict"))).toBe(true);
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(2, "GROUP 2", "GROUP"),
		);
	});

	it("invalidates optimistic tokens when the desk identity changes", () => {
		const store = readyStore();
		const token = store.beginOptimisticCommandLine({ text: "FIXTURE 9" });
		store.reset(SHOW_ID, OTHER_DESK_ID);

		expect(store.commitCommandLine(token, commandLine(2, "FIXTURE 9"))).toBe(
			false,
		);
	});

	it("keeps a local command patch over a newer OSC authority", () => {
		const store = readyStore();
		const token = store.beginOptimisticCommandLine({
			text: "FIXTURE 12",
			pristine: false,
		});
		store.applyChange(
			{ deskId: DESK_ID, commandLine: commandLine(2, "GROUP", "GROUP") },
			15,
		);

		expect(store.getSnapshot().commandLine).toMatchObject({
			text: "FIXTURE 12",
			target: "GROUP",
			pristine: false,
			revision: 2,
		});
		store.rollback(token, new Error("write rejected"));
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(2, "GROUP", "GROUP"),
		);
	});

	it("removes only the failed command operation", () => {
		const store = readyStore();
		const first = store.beginOptimisticCommandLine({ text: "FIXTURE 1" });
		const second = store.beginOptimisticCommandLine({ text: "FIXTURE 12" });
		store.applyChange(
			{ deskId: DESK_ID, commandLine: commandLine(2, "GROUP", "GROUP") },
			18,
		);

		store.rollback(first, new Error("older edit rejected"));
		expect(store.getSnapshot().commandLine?.text).toBe("FIXTURE 12");
		expect(store.getSnapshot().pendingCapabilities).toEqual(
			new Set(["commandLine"]),
		);
		store.commit(second);
		expect(store.getSnapshot().commandLine?.text).toBe("GROUP");
	});

	it("replays remaining ordered selection intent over latest authority", () => {
		const store = readyStore();
		const first = store.beginOptimisticSelectionUpdate((current) => ({
			...current,
			selected: [FIXTURE_2, FIXTURE_1],
			expression: {
				type: "sources",
				items: [{ type: "live_group", groupId: "7" }],
			},
		}));
		const second = store.beginOptimisticSelectionUpdate((current) => ({
			...current,
			selected: [FIXTURE_3],
		}));
		store.applyChange(
			{
				deskId: DESK_ID,
				selection: selection(2, [FIXTURE_1], {
					type: "live_group",
					groupId: "9",
					rule: { type: "all" },
				}),
			},
			22,
		);

		expect(store.getSnapshot().selection).toMatchObject({
			selected: [FIXTURE_3],
			revision: 2,
		});
		expect(selectedGroupId(store.getSnapshot().selection)).toBe("7");

		store.rollback(first, new Error("Group selection rejected"));
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_3]);
		expect(selectedGroupId(store.getSnapshot().selection)).toBe("9");
		expect(store.getSnapshot().pendingCapabilities).toEqual(
			new Set(["selection"]),
		);
		expect(store.commit(second)).toBe(true);
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_1]);
	});

	it("recomputes dependent selection reducers after an earlier rollback", () => {
		const store = readyStore();
		const append = (fixture: string) =>
			store.beginOptimisticSelectionUpdate((current) => ({
				...current,
				selected: [...current.selected, fixture],
			}));
		const first = append(FIXTURE_3);
		const second = append("55555555-5555-4555-8555-555555555555");
		store.applyChange(
			{
				deskId: DESK_ID,
				selection: selection(2, [FIXTURE_2]),
			},
			23,
		);

		store.rollback(first, new Error("first intent failed"));
		expect(store.getSnapshot().selection?.selected).toEqual([
			FIXTURE_2,
			"55555555-5555-4555-8555-555555555555",
		]);
		expect(store.commit(second)).toBe(true);
	});

	it("lets newer event authority beat an older selection response", () => {
		const store = readyStore();
		const token = store.beginOptimisticSelectionUpdate((current) => ({
			...current,
			selected: [FIXTURE_2],
		}));
		store.applyChange(
			{
				deskId: DESK_ID,
				selection: selection(3, [FIXTURE_3]),
			},
			24,
		);

		expect(store.commitSelection(token, selection(2, [FIXTURE_2]))).toBe(
			true,
		);
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_3]);
		expect(store.authoritativeSelectionRevision()).toBe(3);
	});

	it("tracks a semantic reselection even when its projection is unchanged", () => {
		const store = readyStore();
		const token = store.beginOptimisticSelectionUpdate((current) => current);
		expect(token).not.toBeNull();
		expect(store.getSnapshot().pendingCapabilities).toEqual(
			new Set(["selection"]),
		);
	});
});
