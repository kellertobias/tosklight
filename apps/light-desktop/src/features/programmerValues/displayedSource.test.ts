import { describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../api/familyEncoderModels";
import {
	decodeProgrammerPreloadValuesActionOutcome,
	encodeProgrammerPreloadValuesActionRequest,
} from "../../api/programmerPreloadValuesWire";
import {
	decodeProgrammerValuesActionOutcome,
	encodeProgrammerValuesActionRequest,
} from "../../api/programmerValuesWire";
import {
	DisplayedSourceReadouts,
	RECENT_DISPLAYED_SOURCES,
	decodeOutputReadoutSnapshot,
	outputReadoutsPath,
} from "./displayedSource";

const OWNER = "11111111-1111-4111-8111-111111111111";
const SHOW = "33333333-3333-4333-8333-333333333333";
const CORRELATION = "44444444-4444-4444-8444-444444444444";

function snapshot(
	lease: number | null,
	lane: "normal" | "preload" = "normal",
): OutputReadoutSnapshot {
	return {
		lane,
		scope: { show_id: SHOW },
		frame:
			lease === null
				? null
				: { generation: 1, sequence: lease * 10, sampled_at: "2026-10-02T00:00:00Z" },
		lease,
		revision: 4,
		unavailable: lease === null ? "no_accepted_preload" : null,
		owners:
			lease === null
				? []
				: [
						{
							fixture_id: OWNER,
							position: {
								available: true,
								commands: [
									{
										destination: OWNER,
										emitter_id: OWNER,
										pan_degrees: 10,
										tilt_degrees: 20,
									},
								],
								common: { pan_degrees: 10, tilt_degrees: 20 },
							},
						},
					],
	};
}

const OTHER = "22222222-2222-4222-8222-222222222222";

function delivered(
	lease: number,
	owners: readonly string[],
	lane: "normal" | "preload" = "preload",
): OutputReadoutSnapshot {
	const base = snapshot(lease, lane);
	return {
		...base,
		owners: owners.map((fixture_id) => ({ ...base.owners[0], fixture_id })),
	};
}

describe("displayed-source readouts", () => {
	it("reads with the show guard, keeps the delivered lease and names it per lane", async () => {
		const request = vi.fn(async () => snapshot(7));
		const readouts = new DisplayedSourceReadouts({
			request,
			showId: () => SHOW,
		});
		expect(readouts.displayedSource("normal")).toBeNull();
		const read = await readouts.read("normal", [OWNER, OWNER]);
		expect(request).toHaveBeenCalledWith(
			`/api/v2/output/readouts?lane=normal&fixture_ids=${OWNER},${OWNER}`,
			{ headers: { "X-Tosk-Show": SHOW } },
		);
		expect(read.owners[0].position.common).toEqual({
			pan_degrees: 10,
			tilt_degrees: 20,
		});
		expect(readouts.displayedSource("normal")).toEqual({ lane: "normal", lease: 7 });
		expect(readouts.displayedSource("preload")).toBeNull();
	});

	it("shares one in-flight request between duplicate readers and never polls", async () => {
		let resolve!: (value: unknown) => void;
		const request = vi.fn(
			() =>
				new Promise<unknown>((done) => {
					resolve = done;
				}),
		);
		const readouts = new DisplayedSourceReadouts({ request });
		const first = readouts.read("normal", [OWNER]);
		const duplicate = readouts.read("normal", [OWNER]);
		resolve(snapshot(3));
		expect(await first).toBe(await duplicate);
		expect(request).toHaveBeenCalledTimes(1);
		expect(request).toHaveBeenCalledWith(outputReadoutsPath("normal", [OWNER]), {});
	});

	it("never replaces a newer lease with an out-of-order older response", () => {
		const readouts = new DisplayedSourceReadouts({ request: vi.fn() });
		expect(readouts.observe(snapshot(9))).toBe(true);
		expect(readouts.observe(snapshot(8))).toBe(false);
		expect(readouts.displayedSource("normal")?.lease).toBe(9);
		readouts.forget("normal");
		expect(readouts.displayedSource("normal")).toBeNull();
	});

	it("an unavailable Preload readout leaves no lease to name", async () => {
		const readouts = new DisplayedSourceReadouts({
			request: async () => snapshot(null, "preload"),
		});
		readouts.observe(snapshot(5, "preload"));
		const read = await readouts.read("preload", [OWNER]);
		expect(read.unavailable).toBe("no_accepted_preload");
		expect(readouts.displayedSource("preload")).toBeNull();
	});

	it("decodes tolerantly but rejects a malformed lease or owner", () => {
		expect(
			decodeOutputReadoutSnapshot({ ...snapshot(2), future: true }).lease,
		).toBe(2);
		expect(() => decodeOutputReadoutSnapshot({ ...snapshot(2), lease: "2" })).toThrow(
			"$.lease",
		);
		expect(() =>
			decodeOutputReadoutSnapshot({ ...snapshot(2), owners: [{ fixture_id: OWNER }] }),
		).toThrow("$.owners[0].position");
	});

	it("encodes displayed_source on both lanes only when present and decodes the hold", () => {
		const intent = {
			action: "apply_intent" as const,
			fixtureIds: [OWNER],
			attribute: "position",
			operation: { type: "relative_step" as const, delta: 1 },
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		};
		const normal = encodeProgrammerValuesActionRequest({
			requestId: "r1",
			expectedRevision: 1,
			expectedCaptureModeRevision: 1,
			action: { ...intent, displayedSource: { lane: "normal", lease: 7 } },
		});
		expect(normal.action).toMatchObject({
			displayed_source: { lane: "normal", lease: 7 },
		});
		const legacy = encodeProgrammerValuesActionRequest({
			requestId: "r2",
			expectedRevision: 1,
			expectedCaptureModeRevision: 1,
			action: intent,
		});
		expect("displayed_source" in legacy.action).toBe(false);
		const preload = encodeProgrammerPreloadValuesActionRequest({
			requestId: "r3",
			expectedPreloadRevision: 1,
			expectedCaptureModeRevision: 1,
			action: { ...intent, displayedSource: { lane: "preload", lease: 8 } },
		});
		expect(preload.action).toMatchObject({
			displayed_source: { lane: "preload", lease: 8 },
		});
		const held = {
			request_id: "r1",
			correlation_id: CORRELATION,
			revision: 3,
			capture_mode_revision: 1,
			status: "no_change",
			replayed: false,
			hold: "displayed_source_unavailable",
		};
		expect(decodeProgrammerValuesActionOutcome(held).hold).toBe(
			"displayed_source_unavailable",
		);
		expect(decodeProgrammerPreloadValuesActionOutcome(held).hold).toBe(
			"displayed_source_unavailable",
		);
		const { hold: _hold, ...quiet } = held;
		expect("hold" in decodeProgrammerValuesActionOutcome(quiet)).toBe(false);
	});

	it("a second consumer's read never discards the lease of what another consumer shows", () => {
		const readouts = new DisplayedSourceReadouts({ request: vi.fn() });
		expect(readouts.observe(delivered(4, [OTHER]))).toBe(true);
		expect(readouts.observe(delivered(3, [OWNER]))).toBe(false);
		expect(readouts.latest("preload")?.lease).toBe(4);
		expect(readouts.displayedSource("preload", [OWNER])).toEqual({
			lane: "preload",
			lease: 3,
		});
		expect(readouts.displayedSource("preload", [OTHER])?.lease).toBe(4);
		expect(readouts.displayedSource("preload")?.lease).toBe(4);
		readouts.forget("preload", 4);
		expect(readouts.displayedSource("preload")?.lease).toBe(3);
	});

	it("one lease delivered to two consumers covers both and the history stays bounded", () => {
		const readouts = new DisplayedSourceReadouts({ request: vi.fn() });
		readouts.observe(delivered(5, [OWNER], "normal"));
		readouts.observe(delivered(5, [OTHER], "normal"));
		expect(readouts.displayedSource("normal", [OWNER, OTHER])?.lease).toBe(5);
		for (let lease = 6; lease < 6 + RECENT_DISPLAYED_SOURCES * 3; lease += 1)
			readouts.observe(delivered(lease, [], "normal"));
		const newest = 5 + RECENT_DISPLAYED_SOURCES * 3;
		expect(readouts.displayedSource("normal", [OWNER])?.lease).toBe(newest);
		for (let lease = newest; lease > newest - RECENT_DISPLAYED_SOURCES; lease -= 1)
			readouts.forget("normal", lease);
		expect(readouts.displayedSource("normal")).toBeNull();
	});
});
