import { describe, expect, it } from "vitest";
import {
	WireValidationError,
	decodeCommandLineChangedEvent,
	decodeCommandLineResponse,
	decodeCommandOperationOutcome,
	decodeCommandOperationResponse,
} from "./wireValidation";

const commandLine = {
	text: "FIXTURE 1 AT 50",
	target: "FIXTURE",
	pristine: false,
	revision: 7,
	pending_choice: null,
} as const;

const pendingChoice = {
	type: "cue_move_copy",
	operation: "copy",
	command: "CUE 1 COPY CUE 2",
	options: [
		{ id: "plain", label: "Copy", command: "CUE 1 COPY CUE 2" },
		{
			id: "status",
			label: "Copy status",
			command: "CUE 1 COPY CUE 2 STATUS",
		},
	],
	cancel_label: "Cancel",
} as const;

describe("v2 command-line wire validation", () => {
	it("returns valid command lines without cloning them", () => {
		expect(decodeCommandLineResponse(commandLine)).toBe(commandLine);
	});

	it("accepts every operation-outcome variant", () => {
		const accepted = {
			outcome: "accepted",
			action: "executed",
			applied: 2,
			warning: null,
		} as const;
		const choiceRequired = {
			outcome: "choice_required",
			pending_choice: pendingChoice,
		} as const;
		const rejected = { outcome: "rejected", error: "unsupported" } as const;

		expect(decodeCommandOperationOutcome(accepted)).toBe(accepted);
		expect(decodeCommandOperationOutcome(choiceRequired)).toBe(choiceRequired);
		expect(decodeCommandOperationOutcome(rejected)).toBe(rejected);
	});

	it("validates the flattened operation envelope", () => {
		const response = {
			request_id: "request-7",
			outcome: "accepted",
			action: "edited",
			command_line: commandLine,
		} as const;

		expect(decodeCommandOperationResponse(response)).toBe(response);
	});

	it("reports the exact path of malformed nested choice data", () => {
		expect(() =>
			decodeCommandOperationOutcome({
				outcome: "choice_required",
				pending_choice: {
					...pendingChoice,
					options: [{ ...pendingChoice.options[0], id: "invalid" }],
				},
			}),
		).toThrowError("$.pending_choice.options[0].id");
	});

	it("rejects unsafe wire integers before JavaScript can silently round them", () => {
		expect(() =>
			decodeCommandLineResponse({
				...commandLine,
				revision: Number.MAX_SAFE_INTEGER + 1,
			}),
		).toThrowError("$.revision: expected a non-negative safe integer");
	});

	it("rejects missing required fields with a useful path", () => {
		expect(() =>
			decodeCommandOperationResponse({
				request_id: "request-8",
				outcome: "rejected",
				command_line: commandLine,
			}),
		).toThrowError("$.error: expected a string; received undefined");
	});

	it("exposes structured validation errors", () => {
		try {
			decodeCommandLineResponse(null);
			expect.unreachable("null must not decode as a command line");
		} catch (error) {
			expect(error).toBeInstanceOf(WireValidationError);
			expect((error as WireValidationError).path).toBe("$");
		}
	});
});

describe("v2 command-line event wire validation", () => {
	it("accepts the complete event and its omitted optional fields", () => {
		const event = {
			desk_id: "20000000-0000-0000-0000-000000000001",
			session_id: "20000000-0000-0000-0000-000000000002",
			user_id: "20000000-0000-0000-0000-000000000003",
			text: "GROUP",
			target: "GROUP",
			pristine: true,
			revision: 9,
			source: "http_key",
		} as const;

		expect(decodeCommandLineChangedEvent(event)).toBe(event);
		expect(
			decodeCommandLineChangedEvent({
				...event,
				request_id: "request-9",
				redacted: false,
			}),
		).toMatchObject({ request_id: "request-9", redacted: false });
	});

	it("reports malformed event identity and source paths", () => {
		const event = {
			desk_id: "not-a-uuid",
			session_id: "20000000-0000-0000-0000-000000000002",
			user_id: "20000000-0000-0000-0000-000000000003",
			text: "FIXTURE",
			target: "FIXTURE",
			pristine: true,
			revision: 0,
			source: "http",
		};

		expect(() => decodeCommandLineChangedEvent(event)).toThrowError(
			"$.desk_id: expected a hyphenated UUID",
		);
		expect(() =>
			decodeCommandLineChangedEvent({
				...event,
				desk_id: "20000000-0000-0000-0000-000000000001",
				source: "osc",
			}),
		).toThrowError("$.source: expected one of http, http_key");
	});
});
