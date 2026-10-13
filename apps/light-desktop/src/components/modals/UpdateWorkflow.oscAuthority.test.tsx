import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { routeControlSurfaceIntent } from "../../features/controlSurfaceInteraction/registry";
import { createCommandLineTestAuthority } from "../../features/programmingInteraction/testing/commandLineTestAuthority";
import { routeOperatorEvent } from "../../features/server/operatorEventRouting";
import { defaultUpdateSettings } from "../control/updateWorkflow";
import { UpdateWorkflow } from "./UpdateWorkflow";

const workflow = vi.hoisted(() => ({
	state: { updateArmed: false, shiftArmed: false },
	dispatch: vi.fn(),
	update: {
		scopeKey: "osc-authority",
		loadSettings: vi.fn(),
		targets: vi.fn(),
	},
}));
vi.mock("../../features/programmingUpdate/ProgrammingUpdateProvider", () => ({
	useProgrammingUpdate: () => workflow.update,
}));
vi.mock("../../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: vi.fn() }),
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => ({}) }));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: workflow.state, dispatch: workflow.dispatch }),
}));
beforeEach(() => {
	vi.clearAllMocks();
	workflow.state.updateArmed = false;
	workflow.state.shiftArmed = false;
	workflow.update.loadSettings.mockResolvedValue(defaultUpdateSettings);
});
afterEach(cleanup);

function publishCommand(
	authority: ReturnType<typeof createCommandLineTestAuthority>,
	revision: number,
	text: string,
) {
	authority.store.applyChange(
		{
			deskId: authority.deskId,
			commandLine: {
				revision,
				text,
				target: "FIXTURE",
				pristine: text === "FIXTURE",
				pendingChoice: null,
			},
		},
		revision + 10,
	);
}

function notifyOscArmed(
	authority: ReturnType<typeof createCommandLineTestAuthority>,
	armed: boolean,
	deskId = authority.deskId,
) {
	routeOperatorEvent(
		{
			type: "operator_notification",
			notification: {
				type: "update_workflow",
				revision: 9,
				notification: { type: "armed", desk_id: deskId, armed },
			},
		},
		{ desk: { id: authority.deskId } } as Parameters<
			typeof routeOperatorEvent
		>[1],
		{} as Parameters<typeof routeOperatorEvent>[2],
	);
}

async function mount(text = "FIXTURE") {
	const authority = createCommandLineTestAuthority({ text });
	render(authority.wrap(<UpdateWorkflow />));
	await act(authority.settle);
	return authority;
}

describe("OSC-owned Update command authority", () => {
	it("observes arming before delayed authority delivery without echoing a write", async () => {
		const authority = await mount();
		await act(async () => {
			// The server has already armed UPDATE; its scoped command event is delayed.
			notifyOscArmed(authority, true);
			await authority.settle();
			publishCommand(authority, 9, "UPDATE");
			publishCommand(authority, 10, "UPDATE PBK 3.5");
			await authority.settle();
		});
		expect(workflow.dispatch).toHaveBeenCalledWith({
			type: "SET_UPDATE_ARMED",
			value: true,
		});
		expect(authority.writes).toEqual([]);
		expect(authority.store.getSnapshot().commandLine).toMatchObject({
			revision: 10,
			text: "UPDATE PBK 3.5",
		});
	});

	it("does not erase newer digits when an OSC disarm arrives before its reset event", async () => {
		const authority = await mount("UPDATE PBK 3.5");
		await act(async () => {
			notifyOscArmed(authority, false);
			await authority.settle();
		});
		expect(workflow.dispatch).toHaveBeenCalledWith({
			type: "SET_UPDATE_ARMED",
			value: false,
		});
		expect(authority.writes).toEqual([]);
		expect(authority.store.getSnapshot().commandLine?.text).toBe(
			"UPDATE PBK 3.5",
		);
		await act(async () => publishCommand(authority, 2, "FIXTURE"));
		expect(authority.store.getSnapshot().commandLine?.text).toBe("FIXTURE");
	});

	it("ignores another desk's Update notification", async () => {
		const authority = await mount("UPDATE PBK 3.5");
		await act(async () => {
			notifyOscArmed(authority, true, "another-desk");
			notifyOscArmed(authority, false, "another-desk");
			await authority.settle();
		});
		expect(workflow.dispatch).not.toHaveBeenCalled();
		expect(authority.writes).toEqual([]);
		expect(authority.store.getSnapshot().commandLine?.text).toBe(
			"UPDATE PBK 3.5",
		);
	});

	it.each([
		"touch",
		"keyboard",
	] as const)("preserves explicit %s arming and cancellation writes", async (source) => {
		const authority = await mount();
		await act(async () => {
			routeControlSurfaceIntent({ type: "update_armed", source, armed: true });
			await authority.settle();
		});
		await waitFor(() => expect(authority.writes).toHaveLength(1));
		expect(authority.writes[0]).toMatchObject({
			text: "UPDATE ",
			expectedRevision: 1,
		});
		await act(async () => {
			routeControlSurfaceIntent({ type: "update_armed", source, armed: false });
			await authority.settle();
		});
		await waitFor(() => expect(authority.writes).toHaveLength(2));
		expect(authority.writes[1]).toMatchObject({
			text: "",
			expectedRevision: 2,
		});
	});
});
