import { describe, expect, it, vi } from "vitest";
import type { ProgrammingComponentEdit } from "../../api/familyEncoderModels";
import type { ProgrammerPreloadValuesActions } from "../programmerPreloadValues/contracts";
import type { ProgrammerPreloadValuesWriter } from "../programmerPreloadValues/writer";
import type { ProgrammerValuesActions } from "./contracts";
import {
	COLOR_GESTURE_FAMILY,
	colorComponentEdits,
	createColorGestureSession,
	createFocusGestureSession,
	createZoomGestureSession,
	FOCUS_GESTURE_FAMILY,
	focusComponentEdits,
	scalarSet,
	scalarStep,
	ZOOM_GESTURE_FAMILY,
	zoomComponentEdits,
} from "./familyGestureFamilies";
import {
	attachGestureWindowGuards,
	FamilyGestureEditRefusedError,
	type FamilyGestureFamily,
	type FamilyGestureFinishInput,
	type FamilyGestureIntentInput,
	FamilyGestureSession,
	type FamilyGestureSessionOptions,
	type FamilyGestureWriter,
} from "./familyGestureSession";
import {
	attachPositionGestureWindowGuards,
	POSITION_GESTURE_FAMILY,
	type PositionGestureWriter,
	positionAngleStep,
} from "./positionGestureSession";
import { FIXTURE_1 } from "./testFixtures";
import type { ProgrammerValuesWriter } from "./writer";

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };
const CHANNEL = "11111111-1111-4111-8111-111111111111";
const FUNCTION = "22222222-2222-4222-8222-222222222222";

// Compile-time contracts: the mounted writers satisfy the family writer surface, the
// Position writer surface is the family one, and `cancelGesture` is now an optional member of
// both lane Actions interfaces, so view-level fakes without it still compile.
const normalWriterFits = (writer: ProgrammerValuesWriter): FamilyGestureWriter => writer;
const preloadWriterFits = (writer: ProgrammerPreloadValuesWriter): FamilyGestureWriter => writer;
const positionWriterIsFamily = (writer: PositionGestureWriter): FamilyGestureWriter => writer;
const normalCancel: ProgrammerValuesActions["cancelGesture"] = (_undoGroup: string) => 0;
const preloadCancel: ProgrammerPreloadValuesActions["cancelGesture"] = (_undoGroup: string) => 0;
const normalActionsWithoutCancel: Pick<ProgrammerValuesActions, "cancelGesture"> = {};
const preloadActionsWithoutCancel: Pick<ProgrammerPreloadValuesActions, "cancelGesture"> = {};
const normalWriterHasCancel = (writer: ProgrammerValuesWriter): Required<
	Pick<ProgrammerValuesActions, "cancelGesture">
> => writer;
void [
	normalWriterFits,
	preloadWriterFits,
	positionWriterIsFamily,
	normalCancel,
	preloadCancel,
	normalActionsWithoutCancel,
	preloadActionsWithoutCancel,
	normalWriterHasCancel,
];

function idSequence() {
	let next = 0;
	return () => `00000000-0000-4000-8000-${String(++next).padStart(12, "0")}`;
}

type LogEntry =
	| { step: "producer-stop" }
	| { step: "apply"; input: FamilyGestureIntentInput }
	| { step: "cancel"; undoGroup: string }
	| { step: "finish"; input: FamilyGestureFinishInput };

function fakeWriter(log: LogEntry[]): FamilyGestureWriter {
	return {
		applyIntent: vi.fn(async (input: FamilyGestureIntentInput) => {
			log.push({ step: "apply", input });
			return { requestId: input.requestId };
		}),
		cancelGesture: vi.fn((undoGroup: string) => {
			log.push({ step: "cancel", undoGroup });
			return 0;
		}),
		finishGesture: vi.fn(async (input: FamilyGestureFinishInput) => {
			log.push({ step: "finish", input });
			return { status: "no_change" };
		}),
	};
}

function options(log: LogEntry[], onError = vi.fn()): FamilyGestureSessionOptions {
	const writer = fakeWriter(log);
	return { writerFor: () => writer, createId: idSequence(), onError };
}

function start<TChange>(session: FamilyGestureSession<TChange>, log: LogEntry[]) {
	return session.start({
		lane: "normal",
		fixtureIds: [FIXTURE_1],
		timing: TIMING,
		stopProducer: () => log.push({ step: "producer-stop" }),
	})!;
}

interface FamilyCase {
	name: string;
	attribute: string;
	session(options: FamilyGestureSessionOptions): FamilyGestureSession<never>;
	change: unknown;
	edits: ProgrammingComponentEdit[];
}

const families: FamilyCase[] = [
	{
		name: "Position",
		attribute: "position",
		session: (o) => new FamilyGestureSession(POSITION_GESTURE_FAMILY, o) as FamilyGestureSession<never>,
		change: { pan: positionAngleStep(2) },
		edits: [{ kind: "scalar", component: { kind: "pan" }, operation: { kind: "relative", value: 2 } }],
	},
	{
		name: "Focus",
		attribute: "focus",
		session: (o) => createFocusGestureSession(o) as FamilyGestureSession<never>,
		change: { focus: scalarSet(0.4) },
		edits: [{ kind: "scalar", component: { kind: "focus" }, operation: { kind: "set", value: { kind: "value", value: 0.4 } } }],
	},
	{
		name: "Zoom",
		attribute: "zoom",
		session: (o) => createZoomGestureSession(o) as FamilyGestureSession<never>,
		change: { zoom: scalarStep(-1.5) },
		edits: [{ kind: "scalar", component: { kind: "zoom" }, operation: { kind: "relative", value: -1.5 } }],
	},
	{
		name: "Color",
		attribute: "color",
		session: (o) => createColorGestureSession(o) as FamilyGestureSession<never>,
		change: { components: [{ component: "hue", operation: scalarStep(10) }] },
		edits: [
			{ kind: "scalar", component: { kind: "color", component: "hue" }, operation: { kind: "relative", value: 10 } },
		],
	},
];

describe("FamilyGestureSession: one core for every family", () => {
	it.each(families)("$name: edits, then producer-stop, cancel and one Finish on attribute $attribute", async (family) => {
		const log: LogEntry[] = [];
		const onError = vi.fn();
		const session = family.session(options(log, onError));
		expect(session.attribute).toBe(family.attribute);
		const gesture = start(session, log);
		expect(gesture.attribute).toBe(family.attribute);
		await gesture.change(family.change as never);
		expect(gesture.end()).toBe(true);
		expect(gesture.end()).toBe(false);
		await gesture.finished;
		expect(log.map((entry) => entry.step)).toEqual(["apply", "producer-stop", "cancel", "finish"]);
		const apply = log[0] as Extract<LogEntry, { step: "apply" }>;
		expect(apply.input.attribute).toBe(family.attribute);
		expect(apply.input.operation).toEqual({ type: "component_edits", edits: family.edits });
		expect(log[2]).toEqual({ step: "cancel", undoGroup: gesture.undoGroup });
		expect((log[3] as Extract<LogEntry, { step: "finish" }>).input).toEqual({
			requestId: expect.any(String),
			attribute: family.attribute,
			undoGroup: gesture.undoGroup,
		});
		expect(onError).not.toHaveBeenCalled();
	});

	it("Focus and Zoom gestures are separate owners with separate Undo groups and Finishes", async () => {
		const log: LogEntry[] = [];
		const ids = idSequence();
		const writer = fakeWriter(log);
		const focus = createFocusGestureSession({ writerFor: () => writer, createId: ids });
		const zoom = createZoomGestureSession({ writerFor: () => writer, createId: ids });
		const a = start(focus, log);
		const b = start(zoom, log);
		await a.change({ focus: scalarStep(0.01) });
		await b.change({ zoom: scalarSet(30) });
		b.end();
		a.end();
		expect(a.undoGroup).not.toBe(b.undoGroup);
		const finishes = log.filter((entry): entry is Extract<LogEntry, { step: "finish" }> => entry.step === "finish");
		expect(finishes.map((entry) => [entry.input.attribute, entry.input.undoGroup])).toEqual([
			["zoom", b.undoGroup],
			["focus", a.undoGroup],
		]);
	});

	it("a custom family builder that throws is reported once and nothing is sent", () => {
		const log: LogEntry[] = [];
		const onError = vi.fn();
		const family: FamilyGestureFamily<{ bad: boolean }> = {
			attribute: "focus",
			buildEdits: () => {
				throw new FamilyGestureEditRefusedError("nope");
			},
		};
		const gesture = start(new FamilyGestureSession(family, options(log, onError)), log);
		expect(gesture.change({ bad: true })).toBeNull();
		expect(log).toEqual([]);
		expect(onError).toHaveBeenCalledOnce();
	});

	it("an empty change is a quiet refusal for every family", () => {
		expect(focusComponentEdits({})).toEqual([]);
		expect(zoomComponentEdits({})).toEqual([]);
		expect(colorComponentEdits({})).toEqual([]);
		expect(FOCUS_GESTURE_FAMILY.attribute).toBe("focus");
		expect(ZOOM_GESTURE_FAMILY.attribute).toBe("zoom");
		expect(COLOR_GESTURE_FAMILY.attribute).toBe("color");
	});

	it("out-of-domain Focus is rejected by the generated-wire validator, not sent", () => {
		const log: LogEntry[] = [];
		const onError = vi.fn();
		const gesture = start(createFocusGestureSession(options(log, onError)), log);
		expect(gesture.change({ focus: scalarStep(Number.POSITIVE_INFINITY) })).toBeNull();
		expect(log).toEqual([]);
		expect(onError).toHaveBeenCalledOnce();
	});
});

describe("Color family builder", () => {
	it("orders coordinates before orthogonal components", () => {
		expect(
			colorComponentEdits({
				coordinates: { x: 0.3, y: 0.3, z: 0.3 },
				components: [{ component: "white_blend", operation: scalarSet(0.2) }],
			}),
		).toEqual([
			{ kind: "coordinates", xyz: { x: 0.3, y: 0.3, z: 0.3 } },
			{ kind: "scalar", component: { kind: "color", component: "white_blend" }, operation: { kind: "set", value: { kind: "value", value: 0.2 } } },
		]);
	});

	it("builds Direct native edits", () => {
		expect(
			colorComponentEdits({
				native: [{ binding: { channel_id: CHANNEL, function_id: FUNCTION }, operation: { kind: "relative", value: 3 } }],
			}),
		).toEqual([
			{ kind: "native", binding: { channel_id: CHANNEL, function_id: FUNCTION }, operation: { kind: "relative", value: 3 } },
		]);
	});

	it.each([
		["Semantic with Direct", { components: [{ component: "red", operation: scalarSet(1) }], native: [{ binding: { channel_id: CHANNEL, function_id: FUNCTION }, operation: { kind: "set", value: 1 } }] }],
		["recipe with hue", { components: [{ component: "red", operation: scalarSet(1) }, { component: "hue", operation: scalarSet(1) }] }],
		["recipe with coordinates", { coordinates: { x: 1, y: 1, z: 1 }, components: [{ component: "green", operation: scalarSet(1) }] }],
		["coordinates with saturation", { coordinates: { x: 1, y: 1, z: 1 }, components: [{ component: "saturation", operation: scalarSet(1) }] }],
		["duplicate component", { components: [{ component: "uv", operation: scalarSet(1) }, { component: "uv", operation: scalarSet(0) }] }],
	] as const)("refuses %s locally", (_name, change) => {
		expect(() => colorComponentEdits(change as never)).toThrow(FamilyGestureEditRefusedError);
	});
});

describe("Family-neutral window guards", () => {
	it("cancel any family's open gesture on blur, and the Position name is an alias", () => {
		expect(attachPositionGestureWindowGuards).toBe(attachGestureWindowGuards);
		const log: LogEntry[] = [];
		const session = createZoomGestureSession(options(log));
		const window = new EventTarget() as Window;
		const document = Object.assign(new EventTarget(), { hidden: false }) as Document;
		const detach = attachGestureWindowGuards(session, { window, document });
		const gesture = start(session, log);
		window.dispatchEvent(new Event("blur"));
		window.dispatchEvent(new Event("blur"));
		expect(gesture.endReason).toBe("blur");
		expect(log.filter((entry) => entry.step === "finish")).toHaveLength(1);
		detach();
	});
});
