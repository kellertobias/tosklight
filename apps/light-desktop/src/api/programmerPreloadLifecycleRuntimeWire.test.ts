import { describe, expect, it } from "vitest";
import {
	cueProjection,
	SHOW_ID,
	CUE_LIST_ID,
} from "../features/playbackRuntime/testFixtures";
import type {
	PlaybackRuntimeIdentity,
	DynamicPlaybackRuntimeProjection,
	ProgrammingPreloadLifecycleOutcome as WireOutcome,
} from "./generated/light-wire";
import type { ProgrammerPreloadLifecycleRequest } from "../features/programmerPreloadLifecycle/contracts";
import { decodeProgrammerPreloadLifecycleOutcome } from "./programmerPreloadLifecycleWire";

const request: ProgrammerPreloadLifecycleRequest = {
	requestId: "preload-go-owners",
	expectedCaptureModeRevision: 3,
	expectedValuesRevision: 5,
	expectedQueueRevision: 6,
	expectedSelectionRevision: 7,
	action: {
		type: "go",
		showId: SHOW_ID,
		expectedShowRevision: 4,
		expectedPlaybackEventSequence: 10,
	},
};
function response(): WireOutcome {
	const first = cueProjection(11, 0, 2);
	const second = cueProjection(301, 0, 2);
	if (
		first.target !== "cue_list" ||
		!first.runtime ||
		second.target !== "cue_list" ||
		!second.runtime
	)
		throw new Error("running fixture required");
	first.runtime.owner = { kind: "playback", page: 2, position: 11 };
	second.identity = { kind: "playback", page: 2, position: 301 };
	second.runtime.owner = { kind: "playback", page: 2, position: 301 };
	return {
		request_id: request.requestId,
		correlation_id: "55555555-5555-4555-8555-555555555555",
		replayed: false,
		status: "changed",
		active: false,
		capture_mode: {
			revision: 4,
			blind: false,
			preview: false,
			preload_capture_programmer: true,
		},
		capture_mode_event_sequence: 9,
		values_revision: 5,
		queue_revision: 7,
		queue_projection: { revision: 7, actions: [] },
		queue_event_sequence: 13,
		selection_revision: 7,
		commit: {
			show_id: SHOW_ID,
			show_revision: 4,
			playback_event_sequence_before: 10,
			playback_event_sequence_after: 12,
			committed_at: "2026-10-10T01:28:00Z",
			programmer_fade_millis: 0,
			executed_playback_actions: 2,
			executed: [
				{
					address: { page: 2, position: 11 },
					action: "on",
					control_source: { surface_id: "main", control_id: 11 },
				},
				{
					address: { page: 2, position: 301 },
					action: "toggle",
					control_source: { surface_id: "screen-2", control_id: 301 },
				},
			],
			runtime_changes: [
				{ projection: first, event_sequence: 11 },
				{ projection: second, event_sequence: 12 },
			],
		},
	};
}
function firstRuntime(body: WireOutcome) {
	const projection = body.commit!.runtime_changes[0].projection;
	if (projection.target !== "cue_list" || !projection.runtime)
		throw new Error("running fixture required");
	return projection;
}

describe("Preload commit exact runtime owners", () => {
	it("accepts two surface-independent owners in one realistic committed GO response", () => {
		const body = response();
		const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request);
		expect(decoded.commit?.executedPlaybackActions).toBe(2);
		expect(
			decoded.commit?.runtimeChanges.map((item) => item.eventSequence),
		).toEqual([11, 12]);
		const changes = decoded.commit!.runtimeChanges;
		expect(changes[0].projection).toMatchObject({
			identity: { kind: "playback", page: 2, position: 11 },
			runtime: { owner: { kind: "playback", page: 2, position: 11 } },
		});
		expect(changes[1].projection).toMatchObject({
			identity: { kind: "playback", page: 2, position: 301 },
			runtime: { owner: { kind: "playback", page: 2, position: 301 } },
		});
		expect(body.commit?.runtime_changes[0].projection).toEqual(
			changes[0].projection,
		);
	});
	it.each([
		undefined,
		null,
	])("accepts declared optional missing/null owner %s", (owner) => {
		const body = response();
		for (const change of body.commit!.runtime_changes) {
			if (change.projection.target !== "cue_list" || !change.projection.runtime)
				throw new Error("cue");
			if (owner === undefined) delete change.projection.runtime.owner;
			else change.projection.runtime.owner = null;
		}
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).not.toThrow();
	});
	it("keeps an aggregate requested identity distinct from exact Direct activation owner", () => {
		const body = response();
		const projection = firstRuntime(body);
		projection.identity = { kind: "cue_list", cue_list_id: CUE_LIST_ID };
		projection.runtime!.owner = {
			kind: "direct_cue_list",
			cue_list_id: CUE_LIST_ID,
		};
		const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request)
			.commit!.runtimeChanges[0].projection;
		expect(decoded).toMatchObject({
			identity: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			runtime: { owner: { kind: "direct_cue_list", cue_list_id: CUE_LIST_ID } },
		});
	});
	it("accepts a declared Direct request without changing its exact owner", () => {
		const body = response();
		const projection = firstRuntime(body);
		projection.identity = {
			kind: "direct_cue_list",
			cue_list_id: CUE_LIST_ID,
		};
		projection.runtime!.owner = {
			kind: "direct_cue_list",
			cue_list_id: CUE_LIST_ID,
		};
		expect(
			decodeProgrammerPreloadLifecycleOutcome(body, request).commit!
				.runtimeChanges[0].projection,
		).toMatchObject({
			identity: projection.identity,
			runtime: { owner: projection.runtime!.owner },
		});
	});
	it("does not misclassify a GO commit as a successful pending Clear", () => {
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(response(), {
				...request,
				action: { type: "clear_pending" },
			}),
		).toThrow(/\$\.commit: expected absent/);
	});

	it.each([
		{ kind: "playback", page: 2, position: 0 },
		{ kind: "playback", page: 0, position: 301 },
		{ kind: "playback", page: 2, position: 0 },
		{ kind: "undeclared" },
		{ kind: "playback", page: 2, position: 11, foreign: true },
		{ kind: "direct_cue_list", cue_list_id: CUE_LIST_ID, playback_number: 11 },
		{ kind: "playback", page: 2 },
	])("rejects malformed/foreign owner %#", (owner) => {
		const body = response();
		firstRuntime(body).runtime!.owner =
			owner as unknown as PlaybackRuntimeIdentity;
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(/\$\.commit\.runtime_changes\[0\]\.projection\.runtime\.owner/);
	});
	it("continues rejecting undeclared runtime fields", () => {
		const body = response();
		Object.assign(firstRuntime(body).runtime!, { foreign: true });
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(/runtime\.foreign/);
	});
});

describe("TL755 portable commit and compiled runtime authorities", () => {
	function layoutCommit(engineRevision = 664) {
		const body = response();
		const commit = body.commit;
		if (!commit) throw new Error("commit fixture required");
		commit.show_revision = 665;
		for (const change of commit.runtime_changes)
			change.projection.scope.show_revision = engineRevision;
		const layoutRequest: ProgrammerPreloadLifecycleRequest = {
			...request,
			action: {
				type: "go",
				showId: SHOW_ID,
				expectedShowRevision: 665,
				expectedPlaybackEventSequence: 10,
			},
		};
		return { body, commit, layoutRequest };
	}
	it.each([
		664, 660,
	])("accepts a layout-only portable advance with compiled revision %s", (engineRevision) => {
		const { body, layoutRequest } = layoutCommit(engineRevision);
		const first = firstRuntime(body);
		first.identity = { kind: "playback", page: 3, position: 16 };
		if (!first.runtime) throw new Error("runtime fixture required");
		first.runtime.owner = { kind: "playback", page: 3, position: 16 };
		first.runtime.enabled = false;
		const before = structuredClone(body);
		const decoded = decodeProgrammerPreloadLifecycleOutcome(
			body,
			layoutRequest,
		);
		const decodedCommit = decoded.commit;
		if (!decodedCommit) throw new Error("decoded commit required");
		expect(decodedCommit.showRevision).toBe(665);
		expect(
			decodedCommit.runtimeChanges.map(
				(change) => change.projection.scope.show_revision,
			),
		).toEqual([engineRevision, engineRevision]);
		expect(decodedCommit.runtimeChanges[0].projection).toEqual(first);
		expect(decoded.queueProjection?.actions).toEqual([]);
		expect(body).toEqual(before);
	});
	it("rejects a compiled revision newer than portable commit authority", () => {
		const { body, layoutRequest } = layoutCommit(666);
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, layoutRequest),
		).toThrow(/runtime_changes\[0\].projection.scope.show_revision/);
	});
	it("rejects mixed compiled generations within one atomic GO", () => {
		const { body, commit, layoutRequest } = layoutCommit();
		commit.runtime_changes[1].projection.scope.show_revision = 663;
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, layoutRequest),
		).toThrow(/runtime_changes\[1\].projection.scope.show_revision/);
	});
	it("still rejects a foreign runtime show", () => {
		const { body, commit, layoutRequest } = layoutCommit();
		commit.runtime_changes[0].projection.scope.show_id =
			"ffffffff-ffff-4fff-8fff-ffffffffffff";
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, layoutRequest),
		).toThrow(/runtime_changes\[0\].projection.scope.show_id/);
	});
	it("still rejects a stale portable commit receipt", () => {
		const { body, commit, layoutRequest } = layoutCommit();
		commit.show_revision = 666;
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, layoutRequest),
		).toThrow(/\$\.commit.show_revision/);
	});
	it.each([
		10, 13,
	])("still rejects runtime event %s outside the committed interval", (eventSequence) => {
		const { body, commit, layoutRequest } = layoutCommit();
		commit.runtime_changes[0].event_sequence = eventSequence;
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, layoutRequest),
		).toThrow(/runtime_changes\[0\].event_sequence/);
	});
});
function dynamicResponse(active: boolean, embedded: boolean): WireOutcome {
	const body = response();
	for (const change of body.commit!.runtime_changes) {
		const original = change.projection;
		const runtime: DynamicPlaybackRuntimeProjection = {
			owner_address: {
				page: 2,
				position:
					original.identity.kind === "playback"
						? original.identity.position
						: 11,
			},
			enabled: true,
			paused: false,
			flash: false,
			activated_at: "2026-10-10T01:28:00Z",
			fader_value: 1,
			fader_pickup_required: true,
			fader_pickup_target: 0.5,
			size: 0.75,
			master: 1,
			local_speed_numerator: 1,
			local_speed_denominator: 2,
			learned_duration_millis: null,
			state: "active",
			instance_id: "66666666-6666-4666-8666-666666666666",
			controller_id: "77777777-7777-4777-8777-777777777777",
			winning_controller_id: "77777777-7777-4777-8777-777777777777",
			controller_status: "winning",
			target_count: 4,
			compatible_target_count: 3,
			missing_target_count: 0,
			unpatched_target_count: 1,
			lane_count: 2,
			supported_address_count: 6,
			skipped_address_count: 2,
			speed_source: "speed_group",
			effective_speed_multiplier: 0.5,
			effective_duration_millis: 4000,
			warning: "One target is unpatched",
		};
		change.projection = {
			scope: original.scope,
			identity: original.identity,
			target: "dynamic",
			dynamic_id: embedded ? null : "88888888-8888-4888-8888-888888888888",
			last_known_pool_number: 100,
			embedded,
			runtime: active ? runtime : null,
		};
	}
	return body;
}

describe("Preload committed standalone Dynamic projections", () => {
	it.each([
		[true, false],
		[true, true],
		[false, false],
		[false, true],
	])("preserves exact cross-page Playback identity and exact Dynamic runtime active=%s embedded=%s", (active, embedded) => {
		const body = dynamicResponse(active, embedded);
		const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request);
		expect(decoded.commit!.executedPlaybackActions).toBe(2);
		expect(
			decoded.commit!.runtimeChanges.map((change) => change.eventSequence),
		).toEqual([11, 12]);
		expect(
			decoded.commit!.runtimeChanges.map((change) => change.projection),
		).toEqual(body.commit!.runtime_changes.map((change) => change.projection));
		expect(decoded.commit!.runtimeChanges[0].projection.identity).toEqual({
			kind: "playback",
			page: 2,
			position: 11,
		});
		expect(decoded.commit!.runtimeChanges[1].projection.identity).toEqual({
			kind: "playback",
			page: 2,
			position: 301,
		});
	});
	it.each([
		["projection", { foreign: true }, "foreign"],
		["projection", { embedded: "yes" }, "embedded"],
		["projection", { last_known_pool_number: 0 }, "last_known_pool_number"],
		["runtime", { foreign: true }, "foreign"],
		[
			"runtime",
			{ owner: { kind: "playback", page: 2, position: 11 } },
			"owner",
		],
		["runtime", { state: "undeclared" }, "state"],
		["runtime", { playback_number: 0 }, "playback_number"],
		["runtime", { target_count: -1 }, "target_count"],
		["runtime", { controller_id: null }, "controller_id"],
		["runtime", { fader_pickup_required: "yes" }, "fader_pickup_required"],
	])("rejects malformed/foreign Dynamic %s payload %#", (part, patch, field) => {
		const body = dynamicResponse(true, false);
		const projection = body.commit!.runtime_changes[0].projection;
		if (projection.target !== "dynamic" || !projection.runtime)
			throw new Error("Dynamic fixture required");
		Object.assign(part === "runtime" ? projection.runtime : projection, patch);
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(
			new RegExp(
				`\\$\\.commit\\.runtime_changes\\[0\\]\\.projection.*${field}`,
			),
		);
	});
});

function receiptCommit(body: WireOutcome) {
	if (!body.commit)
		throw new Error("accepted GO receipt fixture requires a commit");
	return body.commit;
}
const TIMECODE_ID = "d63061ad-6aad-4701-8442-b6f0d2aca93b";
const MACRO_ID = "80000000-0000-4000-8000-000000000001";
function externalTargetResponse(target: "macro" | "timecode"): WireOutcome {
	const body = response();
	const first = receiptCommit(body).runtime_changes[0];
	first.projection = {
		scope: first.projection.scope,
		identity: { kind: "playback", page: 3, position: 1 },
		...(target === "timecode"
			? { target, timecode_id: TIMECODE_ID }
			: { target, macro_id: MACRO_ID }),
	};
	receiptCommit(body).executed[0] = {
		address: { page: 3, position: 1 },
		action: "go",
		control_source: { surface_id: "hardware:217", control_id: 21702 },
	};
	return body;
}

describe("Preload receipt externally owned target projections", () => {
	it.each(["macro", "timecode"] as const)(
		"decodes a complete accepted %s GO receipt without inventing Cue runtime",
		(target) => {
			const body = externalTargetResponse(target);
			const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request);
			expect(decoded.commit?.runtimeChanges[0]).toEqual({
				eventSequence: 11,
				projection: receiptCommit(body).runtime_changes[0].projection,
			});
			expect(decoded.commit?.executed[0]).toMatchObject({
				address: { page: 3, position: 1 },
				action: "go",
			});
			expect(decoded.commit?.runtimeChanges[0].projection).not.toHaveProperty(
				"runtime",
			);
		},
	);
	for (const target of ["macro", "timecode"] as const) {
		const field = target === "macro" ? "macro_id" : "timecode_id";
		it.each([undefined, null, "", 17, {}])(
			`refuses missing or malformed ${field}: %s`,
			(value) => {
				const body = externalTargetResponse(target);
				const projection = receiptCommit(body).runtime_changes[0]
					.projection as unknown as Record<string, unknown>;
				if (value === undefined) delete projection[field];
				else projection[field] = value;
				expect(() =>
					decodeProgrammerPreloadLifecycleOutcome(body, request),
				).toThrow(new RegExp(`projection\\.${field}`));
			},
		);
		it(`keeps exact ${target} shape and current receipt scope strict`, () => {
			for (const extra of ["runtime", "foreign"]) {
				const body = externalTargetResponse(target);
				Object.assign(receiptCommit(body).runtime_changes[0].projection, {
					[extra]: {},
				});
				expect(() =>
					decodeProgrammerPreloadLifecycleOutcome(body, request),
				).toThrow(new RegExp(`projection\\.${extra}`));
			}
			const stale = externalTargetResponse(target);
			receiptCommit(stale).runtime_changes[0].projection.scope.show_revision =
				3;
			expect(() =>
				decodeProgrammerPreloadLifecycleOutcome(stale, request),
			).toThrow(/projection.scope.show_revision/);
		});
	}
	it("still refuses an unknown future target", () => {
		const body = externalTargetResponse("timecode");
		Object.assign(receiptCommit(body).runtime_changes[0].projection, {
			target: "future_transport",
		});
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(/projection.target/);
	});
});

function canonicalTargets() {
	const cue = firstRuntime(response());
	const dynamic = receiptCommit(dynamicResponse(true, false)).runtime_changes[0]
		.projection;
	if (dynamic.target !== "dynamic") throw new Error("Dynamic fixture required");
	return {
		missing: { target: "missing" },
		cue_list: {
			target: "cue_list",
			cue_list_id: cue.cue_list_id,
			runtime: cue.runtime,
		},
		dynamic,
		macro: { target: "macro", macro_id: MACRO_ID },
		timecode: { target: "timecode", timecode_id: TIMECODE_ID },
		group: {
			target: "group",
			group_id: "91",
			master: 0.75,
			flash_level: 0,
			fader_position: 0.5,
			fader_pickup_required: true,
			fader_pickup_target: 0.75,
		},
		speed_group: {
			target: "speed_group",
			group: "A",
			runtime: {
				manual_bpm: 120,
				sound_bpm: null,
				effective_bpm: 120,
				source: "manual",
				sound_status: { status: "disabled" },
				paused: false,
				phase_advancing: true,
				speed_master_scale: 1,
				sound_multiplier: 1,
				source_available: false,
				usable_signal: false,
				input_level: 0,
				selected_band_level: 0,
				synchronized_with: null,
				phase_origin_millis: 0,
				beat_phase: 0.5,
			},
		},
		grand_master: {
			target: "grand_master",
			runtime: {
				level: 1,
				effective_level: 1,
				blackout: false,
				flash_active: false,
				dynamics_paused: false,
			},
		},
		programmer_fade: { target: "programmer_fade", millis: 3000 },
		cue_fade: { target: "cue_fade", millis: 2000 },
	} satisfies Record<
		import("./generated/light-wire").PlaybackTargetProjection["target"],
		import("./generated/light-wire").PlaybackTargetProjection
	>;
}

describe("Preload strict receipt covers the current generated target contract", () => {
	it.each(Object.entries(canonicalTargets()))(
		"accepts all declared fields of canonical %s in a full committed receipt",
		(_target, target) => {
			const body = response();
			const change = receiptCommit(body).runtime_changes[0];
			change.projection = {
				scope: change.projection.scope,
				identity: change.projection.identity,
				...target,
			};
			const decoded = decodeProgrammerPreloadLifecycleOutcome(body, request);
			expect(decoded.commit?.runtimeChanges[0].projection).toEqual(
				change.projection,
			);
		},
	);
	it.each([
		["fader_position", "half"],
		["fader_pickup_required", 1],
		["fader_pickup_target", "three quarters"],
	])("keeps malformed Group %s strict", (field, value) => {
		const body = response();
		const change = receiptCommit(body).runtime_changes[0];
		change.projection = {
			scope: change.projection.scope,
			identity: change.projection.identity,
			...canonicalTargets().group,
		};
		Object.assign(change.projection, { [field]: value });
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(new RegExp(`projection\\.${field}`));
	});
	it("keeps unknown Group fields strict", () => {
		const body = response();
		const change = receiptCommit(body).runtime_changes[0];
		change.projection = {
			scope: change.projection.scope,
			identity: change.projection.identity,
			...canonicalTargets().group,
		};
		Object.assign(change.projection, { foreign: true });
		expect(() =>
			decodeProgrammerPreloadLifecycleOutcome(body, request),
		).toThrow(/projection.foreign/);
	});
});
