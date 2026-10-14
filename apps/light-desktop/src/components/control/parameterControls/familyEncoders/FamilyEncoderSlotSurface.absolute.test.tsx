import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeStore } from "../../../../features/programmerCaptureMode/store";
import { captureModeSnapshot } from "../../../../features/programmerCaptureMode/testFixtures";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "../../../../features/programmerValues/contracts";
import { ProgrammerValuesStore } from "../../../../features/programmerValues/store";
import {
	fixtureValue,
	SESSION_ID,
	SHOW_ID,
	valuesProjection,
	valuesSnapshot,
} from "../../../../features/programmerValues/testFixtures";
import { ProgrammerValuesWriter } from "../../../../features/programmerValues/writer";
import type { ParameterController } from "../useParameterController";
import { FamilyEncoderSlotSurface } from "./FamilyEncoderSlotSurface";
import { FamilyEncoderBinding } from "./familyEncoderBinding";
import { FOCUS, fakeWriter, ZOOM } from "./familyEncoderTestSupport";

afterEach(cleanup);

function rig(hardware: boolean, kind: "zoom" | "focus") {
	const writer = fakeWriter();
	const binding = new FamilyEncoderBinding({ writerFor: () => writer });
	const slot = {
		...(kind === "zoom" ? ZOOM : FOCUS),
		label: kind === "zoom" ? "Zoom" : "Focus",
		limits: kind === "zoom" ? { min: 10, max: 45 } : { min: 0, max: 1 },
	};
	let shown = kind === "zoom" ? 20 : 0.5;
	const controller = () =>
		({
			hardwareConnected: hardware,
			canWriteValues: true,
			hasProgrammerValue: () => true,
			releaseParameter: async () => undefined,
			familyEncoders: {
				componentSlot: () => slot,
				display: () => ({
					value: shown,
					text: String(shown),
					source: "requested",
				}),
				pointChoices: [],
				finishGestures: () => binding.finishGestures(),
				step: () => undefined,
				set: (_index: number, value: number) =>
					binding.set(slot, value, {
						lane: "normal",
						groupId: null,
						timing: { fade: false, fadeMillis: null, delayMillis: null },
					}),
				setRange: (_index: number, values: number[]) =>
					binding.spread(slot, values, {
						lane: "normal",
						groupId: null,
						timing: { fade: false, fadeMillis: null, delayMillis: null },
					}),
			},
		}) as unknown as ParameterController;
	return {
		writer,
		binding,
		controller,
		setShown: (value: number) => {
			shown = value;
		},
	};
}

function open(hardware: boolean, label: string) {
	fireEvent.click(
		screen.getByRole("button", {
			name: hardware
				? new RegExp(`^Encoder 2: ${label},`)
				: `Set Enc 2 · ${label} value`,
		}),
	);
	return screen.getByRole("dialog", {
		name: hardware ? "Encoder 2 value" : `Enc 2 · ${label} value`,
	});
}

describe("absolute family encoder numeric confirmation", () => {
	it.each([
		[false, "keypad"],
		[true, "keypad"],
		[false, "keyboard"],
		[true, "keyboard"],
	] as const)("commits entered Zoom40 after previously requested20 and old-source refresh (hardware %s, %s)", async (hardware, input) => {
		const r = rig(hardware, "zoom");
		const view = render(
			<FamilyEncoderSlotSurface controller={r.controller()} index={1} />,
		);
		const dialog = open(hardware, "Zoom");
		if (input === "keypad") {
			fireEvent.click(within(dialog).getByRole("button", { name: "4" }));
			fireEvent.click(within(dialog).getByRole("button", { name: "0" }));
		} else {
			fireEvent.keyDown(window, { key: "4", code: "Digit4" });
			fireEvent.keyDown(window, { key: "0", code: "Digit0" });
		}
		// A readout refresh carrying the still-current20 does not replace the operator's draft40.
		view.rerender(
			<FamilyEncoderSlotSurface controller={r.controller()} index={1} />,
		);
		fireEvent.click(within(dialog).getByRole("button", { name: "ENTER" }));
		await waitFor(() => expect(r.writer.applyIntent).toHaveBeenCalledOnce());
		expect(r.writer.applyIntent.mock.calls[0][0]).toMatchObject({
			attribute: "zoom",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: "zoom" },
						operation: { kind: "set", value: { kind: "value", value: 40 } },
					},
				],
			},
		});
		r.setShown(40);
		view.rerender(
			<FamilyEncoderSlotSurface controller={r.controller()} index={1} />,
		);
		open(hardware, "Zoom");
		expect(screen.getByRole("textbox")).toHaveValue("40");
		r.binding.dispose();
	});
	it.each([
		false,
		true,
	])("keeps Focus percentage independent (hardware %s)", async (hardware) => {
		const r = rig(hardware, "focus");
		render(<FamilyEncoderSlotSurface controller={r.controller()} index={1} />);
		const dialog = open(hardware, "Focus");
		for (const digit of ["1", "0", "0"])
			fireEvent.click(within(dialog).getByRole("button", { name: digit }));
		fireEvent.click(within(dialog).getByRole("button", { name: "ENTER" }));
		await waitFor(() => expect(r.writer.applyIntent).toHaveBeenCalledOnce());
		expect(r.writer.applyIntent.mock.calls[0][0]).toMatchObject({
			attribute: "focus",
			operation: {
				type: "component_edits",
				edits: [
					{
						kind: "scalar",
						component: { kind: "focus" },
						operation: { kind: "set", value: { kind: "value", value: 1 } },
					},
				],
			},
		});
		r.binding.dispose();
	});
});

function changedAuthorityWriter() {
	const store = new ProgrammerValuesStore();
	store.reset(SHOW_ID, SESSION_ID, "session-a");
	store.installSnapshot(valuesSnapshot({ fixtureValues: [] }));
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
	captureModeStore.installSnapshot(captureModeSnapshot());
	let revision = 1;
	const requests: ProgrammerValuesActionRequest[] = [];
	const applyAction = vi.fn(
		async (
			_scope: { showId: string },
			request: ProgrammerValuesActionRequest,
		): Promise<ProgrammerValuesActionOutcome> => {
			requests.push(request);
			const base = {
				requestId: request.requestId,
				correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
				captureModeRevision: 1,
				replayed: false,
				warning: null,
			};
			if (request.action.action === "finish_gesture")
				return { ...base, status: "no_change", revision };
			expect(request.expectedRevision).toBe(revision);
			const action = request.action;
			if (
				action.action !== "apply_intent" ||
				action.operation.type !== "component_edits"
			)
				throw new Error("unexpected action");
			const edit = action.operation.edits[0];
			if (
				edit?.kind !== "scalar" ||
				edit.operation.kind !== "set" ||
				edit.operation.value.kind !== "value"
			)
				throw new Error("unexpected scalar");
			const value = edit.operation.value.value;
			const old = store.getSnapshot().projection?.fixtureValues ?? [];
			const projection = valuesProjection({
				revision: ++revision,
				fixtureValues: [
					...old.filter((entry) => entry.attribute !== action.attribute),
					fixtureValue(value, {
						attribute: action.attribute,
						value:
							action.attribute === "zoom"
								? {
										kind: "zoom",
										value: {
											convention: "beam",
											opening_degrees: { kind: "value", value },
										},
									}
								: { kind: "normalized", value },
					}),
				],
			});
			return {
				...base,
				status: "changed",
				revision,
				projection,
				eventSequence: revision + 10,
			};
		},
	);
	const onError = vi.fn();
	const writer = new ProgrammerValuesWriter({
		scope: { showId: SHOW_ID },
		store,
		captureModeStore,
		applyAction,
		repair: async () => undefined,
		repairCaptureMode: async () => undefined,
		onError,
	});
	return { writer, store, requests, onError };
}

it("reopens and commits Zoom20 then40 through real gesture/FIFO/changed authority, with changed authority", async () => {
	const server = changedAuthorityWriter();
	const r = rig(false, "zoom");
	r.writer.applyIntent.mockImplementation(
		(input) =>
			server.writer.applyIntent(
				input as Parameters<ProgrammerValuesWriter["applyIntent"]>[0],
			) as never,
	);
	r.writer.finishGesture.mockImplementation(
		(input) =>
			server.writer.finishGesture(
				input as Parameters<ProgrammerValuesWriter["finishGesture"]>[0],
			) as never,
	);
	const view = render(
		<FamilyEncoderSlotSurface controller={r.controller()} index={1} />,
	);
	for (const degrees of [20, 40]) {
		const dialog = open(false, "Zoom");
		for (const digit of String(degrees))
			fireEvent.click(within(dialog).getByRole("button", { name: digit }));
		fireEvent.click(within(dialog).getByRole("button", { name: "ENTER" }));
		await waitFor(() =>
			expect(server.requests).toHaveLength(degrees === 20 ? 2 : 4),
		);
		expect(
			server.store.getSnapshot().projection?.fixtureValues[0]?.value,
		).toEqual({
			kind: "zoom",
			value: {
				convention: "beam",
				opening_degrees: { kind: "value", value: degrees },
			},
		});
		r.setShown(degrees);
		view.rerender(
			<FamilyEncoderSlotSurface controller={r.controller()} index={1} />,
		);
	}
	expect(server.requests.map((request) => request.expectedRevision)).toEqual([
		1, 2, 2, 3,
	]);
	expect(server.requests.map((request) => request.action.action)).toEqual([
		"apply_intent",
		"finish_gesture",
		"apply_intent",
		"finish_gesture",
	]);
	expect(server.requests[0]?.requestId).not.toBe(server.requests[2]?.requestId);
	open(false, "Zoom");
	expect(screen.getByRole("textbox")).toHaveValue("40");
	expect(
		server.onError.mock.calls.filter(([error]) => error instanceof Error),
	).toEqual([]);
	r.binding.dispose();
	server.writer.stop();
});
