import { describe, expect, it, vi } from "vitest";
import { ProgrammingApiClient } from "./programming";
import type { LiveClientTransport } from "./transport";

const DESK_ID = "11111111-1111-4111-8111-111111111111";
const FIXTURE_ID = "22222222-2222-4222-8222-222222222222";
const USER_ID = "44444444-4444-4444-8444-444444444444";

function commandLine(revision = 4) {
	return {
		text: "FIXTURE 7",
		target: "FIXTURE",
		pristine: false,
		revision,
		pending_choice: null,
	};
}

function interactionSnapshot() {
	return {
		cursor: { sequence: 12 },
		projection: {
			desk_id: DESK_ID,
			command_line: commandLine(),
			selection: {
				selected: [FIXTURE_ID],
				expression: { type: "static" },
				revision: 3,
				gesture_open: false,
			},
		},
	};
}

function decodedCommandLine(revision = 4) {
	return {
		text: "FIXTURE 7",
		target: "FIXTURE",
		pristine: false,
		revision,
		pendingChoice: null,
	};
}

function decodedInteractionSnapshot() {
	return {
		cursor: 12,
		projection: {
			deskId: DESK_ID,
			commandLine: decodedCommandLine(),
			selection: {
				selected: [FIXTURE_ID],
				expression: { type: "static" },
				revision: 3,
				gestureOpen: false,
			},
		},
	};
}

function clientReturning(value: unknown) {
	const request = vi.fn(async (_path: string, _init?: RequestInit) => value);
	const sendAction = vi.fn(
		async (_action: unknown, _requestId: string) => value,
	);
	const transport = {
		request,
		sendAction,
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} as unknown as LiveClientTransport;
	return {
		client: new ProgrammingApiClient(transport),
		request,
		sendAction,
	};
}

describe("ProgrammingApiClient v2 interaction boundary", () => {
	it("sends one correlated Programmer values command frame", async () => {
		const response = {
			request_id: "values-1",
			correlation_id: "33333333-3333-4333-8333-333333333333",
			revision: 2,
			capture_mode_revision: 1,
			status: "no_change",
			replayed: false,
			warning: null,
		};
		const { client, sendAction } = clientReturning(response);

		await expect(
			client.programmerValuesLiveAction(USER_ID, {
				requestId: "values-1",
				expectedRevision: 2,
				expectedCaptureModeRevision: 1,
				action: { action: "clear" },
			}),
		).resolves.toMatchObject({ requestId: "values-1", status: "no_change" });
		expect(sendAction).toHaveBeenCalledOnce();
		expect(sendAction).toHaveBeenCalledWith(
			{
				type: "programming_values",
				request: {
					request_id: "values-1",
					expected_revision: 2,
					expected_capture_mode_revision: 1,
					action: { type: "clear" },
				},
			},
			"values-1",
		);
	});

	it("loads a strictly validated desk interaction snapshot", async () => {
		const { client, request } = clientReturning(interactionSnapshot());

		await expect(
			client.programmingInteractionSnapshot(DESK_ID),
		).resolves.toEqual(decodedInteractionSnapshot());
		expect(request).toHaveBeenCalledWith(
			"/api/v2/programming-interaction/snapshot",
			{ headers: { "x-tosk-desk": DESK_ID } },
		);
	});

	it("rejects a snapshot belonging to another desk", async () => {
		const value = interactionSnapshot();
		value.projection.desk_id = "99999999-9999-4999-8999-999999999999";
		const { client } = clientReturning(value);

		await expect(
			client.programmingInteractionSnapshot(DESK_ID),
		).rejects.toThrow("requested desk");
	});

	it("replaces command text with optimistic concurrency", async () => {
		const { client, sendAction } = clientReturning(commandLine(5));

		await expect(
			client.replaceProgrammingCommandLine(DESK_ID, "FIXTURE 8", 4),
		).resolves.toEqual(decodedCommandLine(5));
		expect(sendAction).toHaveBeenCalledOnce();
		const [action, requestId] = sendAction.mock.calls[0];
		expect(action).toEqual({
			type: "command_line_replace",
			request: {
				expected_revision: 4,
				text: "FIXTURE 8",
			},
		});
		expect(requestId).toEqual(expect.any(String));
	});

	it("sends a typed selection action frame and validates its authority", async () => {
		const response = {
			request_id: "selection-1",
			correlation_id: "33333333-3333-4333-8333-333333333333",
			action: "replaced",
			applied: 1,
			selection: interactionSnapshot().projection.selection,
			event_sequence: 13,
			replayed: false,
		};
		const { client, sendAction } = clientReturning(response);

		await expect(
			client.applyProgrammingSelection(DESK_ID, {
				requestId: "selection-1",
				action: {
					type: "replace",
					fixtures: [FIXTURE_ID],
					expectedRevision: 2,
				},
			}),
		).resolves.toMatchObject({
			requestId: "selection-1",
			action: "replaced",
			selection: { selected: [FIXTURE_ID], gestureOpen: false },
			eventSequence: 13,
			warning: null,
		});
		expect(sendAction).toHaveBeenCalledWith(
			{
				type: "programming_selection",
				request: {
					request_id: "selection-1",
					action: "replace",
					fixtures: [FIXTURE_ID],
					expected_revision: 2,
				},
			},
			"selection-1",
		);
	});

	it("generates fixture presets through the persisted HTTP intent boundary", async () => {
		const requestId = "55555555-5555-4555-8555-555555555555";
		const { client, request, sendAction } = clientReturning({
			request_id: requestId,
			correlation_id: "66666666-6666-4666-8666-666666666666",
			show_revision: 8,
			event_sequence: 12,
			created: [],
		});
		vi.spyOn(crypto, "randomUUID").mockReturnValueOnce(requestId);

		await expect(
			client.generateFixturePresets([FIXTURE_ID], 7),
		).resolves.toEqual({
			created: [],
			showRevision: 8,
		});
		expect(request).toHaveBeenCalledWith(
			"/api/v2/preset-profile-generation/update",
			{
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({
					request_id: requestId,
					expected_show_revision: 7,
					fixture_ids: [FIXTURE_ID],
				}),
			},
		);
		expect(sendAction).not.toHaveBeenCalled();
	});
});
