import { describe, expect, it, vi } from "vitest";
import { ProgrammingApiClient } from "./programming";
import type { LiveClientTransport } from "./transport";

const DESK_ID = "11111111-1111-4111-8111-111111111111";
const FIXTURE_ID = "22222222-2222-4222-8222-222222222222";

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
			},
		},
	};
}

function clientReturning(value: unknown) {
	const request = vi.fn(async (_path: string, _init?: RequestInit) => value);
	const transport = {
		request,
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
		command: vi.fn(),
	} as unknown as LiveClientTransport;
	return { client: new ProgrammingApiClient(transport), request };
}

describe("ProgrammingApiClient v2 interaction boundary", () => {
	it("loads a strictly validated desk interaction snapshot", async () => {
		const { client, request } = clientReturning(interactionSnapshot());

		await expect(client.programmingInteractionSnapshot(DESK_ID)).resolves.toEqual(
			decodedInteractionSnapshot(),
		);
		expect(request).toHaveBeenCalledWith(
			`/api/v2/desks/${DESK_ID}/programming-interaction/snapshot`,
		);
	});

	it("rejects a snapshot belonging to another desk", async () => {
		const value = interactionSnapshot();
		value.projection.desk_id = "99999999-9999-4999-8999-999999999999";
		const { client } = clientReturning(value);

		await expect(client.programmingInteractionSnapshot(DESK_ID)).rejects.toThrow(
			"requested desk",
		);
	});

	it("replaces command text with optimistic concurrency", async () => {
		const { client, request } = clientReturning(commandLine(5));

		await expect(
			client.replaceProgrammingCommandLine(DESK_ID, "FIXTURE 8", 4),
		).resolves.toEqual(decodedCommandLine(5));
		const [path, init] = request.mock.calls[0];
		expect(path).toBe(`/api/v2/desks/${DESK_ID}/command-line`);
		expect(init).toBeDefined();
		if (!init) throw new Error("expected a command-line request");
		expect(init.method).toBe("PUT");
		expect(new Headers(init.headers).get("if-match")).toBe("4");
		expect(JSON.parse(String(init.body))).toEqual({ text: "FIXTURE 8" });
	});
});
