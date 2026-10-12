import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiRequestError } from "../../api/ApiRequestError";
import {
	COMMAND_ERROR_EVENT,
	commandErrorFeedback,
	commandErrorSummary,
	reportProgrammingCommandError,
} from "./commandError";

afterEach(() => vi.restoreAllMocks());
describe("command rejection classification", () => {
	it("keeps the actual empty Preset-family rejection brief without hiding faults", () => {
		const message =
			"the normal Programmer has no values in the requested Preset family";
		const dispatch = vi.spyOn(window, "dispatchEvent");
		for (const feedback of [message, `invalid data: ${message}`]) {
			reportProgrammingCommandError(new Error(feedback));
			expect((dispatch.mock.lastCall?.[0] as CustomEvent).detail).toEqual({
				message: feedback,
				kind: "operator",
			});
			expect(commandErrorFeedback(feedback).kind).toBe("operator");
			expect(commandErrorSummary(feedback)).toBe("No Programmer Values");
		}
		for (const suffix of [": database unavailable", "\nnetwork failure"]) {
			expect(commandErrorFeedback(message + suffix).kind).toBe("unexpected");
		}
	});

	it.each([
		"RECORD + and RECORD - currently require GROUP or PBK ... CUE targets",
		"RECORD + and RECORD - require an explicit CUE target",
	])("keeps exact unsupported Record target grammar brief: %s", (message) => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		reportProgrammingCommandError(new Error(message));
		expect((dispatch.mock.lastCall![0] as CustomEvent).detail).toEqual({
			message,
			kind: "operator",
		});
		expect(commandErrorFeedback(message).kind).toBe("operator");
		expect(commandErrorSummary(message)).toBe("Invalid Command");
		for (const suffix of [": database unavailable", "\nnetwork failure"]) {
			expect(commandErrorFeedback(message + suffix).kind).toBe("unexpected");
		}
	});

	it("keeps exact timing grammar rejections brief without hiding unexpected faults", () => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		for (const message of [
			"TIME and DELAY require seconds",
			"TIME and DELAY must be within 0-86400 seconds",
			"time decimal requires digits after the dot",
			"Timing ranges are not supported",
		]) {
			expect(commandErrorFeedback(message).kind).toBe("operator");
			reportProgrammingCommandError(new Error(message));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail.kind).toBe(
				"operator",
			);
		}
		for (const message of [
			"TIME and DELAY require seconds: database unavailable",
			"TIME and DELAY require seconds\nnetwork failure",
			"unexpected timing service failure",
		]) {
			expect(commandErrorFeedback(message).kind).toBe("unexpected");
		}
	});
	it.each([
		"Preset Move and Copy require <family> PRESET <number> on both sides",
		"Preset Move and Copy require PRESET before the number",
	])("keeps actual transfer grammar rejections brief: %s", (message) => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		// Native resolveCommand rejects with this status-less Error; history shares it.
		for (const feedback of [message, `invalid data: ${message}`]) {
			reportProgrammingCommandError(new Error(feedback));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail).toEqual({
				message: feedback,
				kind: "operator",
			});
			expect(commandErrorFeedback(feedback).kind).toBe("operator");
			expect(commandErrorSummary(feedback)).toBe("Invalid Command");
		}
		for (const status of [401, 403, 500]) {
			reportProgrammingCommandError(new ApiRequestError(message, status));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail.kind).toBe(
				"unexpected",
			);
		}
		for (const fault of [
			`${message}: database unavailable`,
			`${message}: authentication failed`,
			"Preset Move and Copy execution task failed",
		]) {
			reportProgrammingCommandError(new Error(fault));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail.kind).toBe(
				"unexpected",
			);
		}
	});
	it.each([
		"- requires a fixture selection",
		"- requires fixture selections on both sides",
		"Programmer Align requires a non-empty selection",
	])("presents the actual selection-required websocket rejection: %s", (message) => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		reportProgrammingCommandError(new Error(message));
		expect((dispatch.mock.lastCall![0] as CustomEvent).detail).toEqual({
			message,
			kind: "operator",
		});
		expect(commandErrorSummary(message)).toBe("No Selection");
		expect(commandErrorFeedback(`invalid data: ${message}`).kind).toBe(
			"operator",
		);
	});
	it.each([
		"level",
		"selection",
		"subset",
		"playback address",
		"playback selection",
		"recording target",
		"Cue transfer address",
		"Cue deletion address",
		"BPM value",
		"Cue number",
		"cue target",
		"Link source",
		"Link destination",
	])("keeps the parser's trailing-token rejection brief: %s", (operand) => {
		const message = `unexpected tokens after ${operand}`;
		const dispatch = vi.spyOn(window, "dispatchEvent");
		for (const feedback of [message, `invalid data: ${message}`]) {
			reportProgrammingCommandError(new Error(feedback));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail).toEqual({
				message: feedback,
				kind: "operator",
			});
			expect(commandErrorFeedback(feedback)).toEqual({
				message: feedback,
				kind: "operator",
			});
		}
	});
	it.each([
		"expected a fixture number",
		"AT requires a level",
		"expected OFFSET or + before the subset offset",
		"expected a fixture reference before +",
		"expected + between fixture ranges",
		"expected a fixture reference after +",
		"expected SET DYNAMIC <pool number>",
		"expected SET CUELIST <positive pool number>",
		"expected SET GROUP <group-number>",
		"SET requires an editable value or object",
		"ASSIGN requires a source before AT",
		"expected ASSIGN GROUP <number> AT PBK <address>",
		"expected ASSIGN <CUELIST|DYNAMIC|MACRO|TIMECODE> <number> AT PBK <address>",
		"DMX requires one universe.address",
		"DIV requires a positive number",
		"OFFSET must be smaller than DIV",
		"offset is invalid",
		"+ requires an offset",
		"DYNAMIC requires a pool number",
		"expected DYNAMIC <number>, OFF, SIZE/SPEED/PHASE AT, or PHASE helpers",
		"DOT requires another cue part",
		"LINK requires AT CUE <destination>",
		"LINK requires CUE or SET playback CUE source",
		"AT requires a BPM value",
		"expected PBK playback address",
		"expected SPD GRP <1-5> AT <BPM | +/- BPM | SPD GRP <1-5>>",
		"expected CUE, CUELIST, or PBK recording target",
		"expected [SET] MACRO|TIMECODE <positive pool number> [+|-], or SET CUELIST <positive pool number>",
		"expected CUELIST <number> AT SPD GRP <1-5>",
		"expected DYNAMIC <number> AT SPD GRP <1-5>",
	])("classifies confirmed input grammar rejection: %s", (message) => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		for (const feedback of [message, `invalid data: ${message}`]) {
			reportProgrammingCommandError(new Error(feedback));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail).toEqual({
				message: feedback,
				kind: "operator",
			});
			expect(commandErrorFeedback(feedback).kind).toBe("operator");
		}
		for (const status of [401, 403, 500]) {
			reportProgrammingCommandError(new ApiRequestError(message, status));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail.kind).toBe(
				"unexpected",
			);
		}
	});
	it("does not disguise unexpected faults as parser feedback", () => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		for (const message of [
			"unexpected runtime failure",
			"expected a fixture number: database unavailable",
			"AT requires a level: authentication failed",
			"expected server response",
			"expected a fixture number from runtime snapshot",
			"unexpected tokens after server response",
			"unexpected tokens after level: database unavailable",
			"unexpected tokens after level\nnetwork failure",
			"OFFSET must be smaller than DIV: database unavailable",
			"OFFSET must be smaller than DIV\nnetwork failure",
			"offset is invalid: authentication failed",
			"+ requires an offset from runtime snapshot",
		]) {
			expect(commandErrorFeedback(message).kind).toBe("unexpected");
			reportProgrammingCommandError(new Error(message));
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail.kind).toBe(
				"unexpected",
			);
		}
		for (const status of [401, 403, 500]) {
			reportProgrammingCommandError(
				new ApiRequestError("unexpected tokens after level", status),
			);
			expect((dispatch.mock.lastCall![0] as CustomEvent).detail.kind).toBe(
				"unexpected",
			);
		}
	});
	it("classifies status-less websocket preset rejections without hiding unknown failures", () => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		// resolveCommand rejects with a plain Error carrying only response.error.
		reportProgrammingCommandError(new Error("preset 2.999 does not exist"));
		expect((dispatch.mock.lastCall![0] as CustomEvent).detail).toEqual({
			message: "preset 2.999 does not exist",
			kind: "operator",
		});
		for (const error of [
			new Error("unrecognised runtime failure"),
			new Error("Command requires authentication"),
			new Error("Preset recording service task failed: database unavailable"),
			new ApiRequestError("preset 2.999 does not exist", 500),
			new ApiRequestError("preset 2.999 does not exist", 401),
			new ApiRequestError("preset 2.999 does not exist", 403),
		]) {
			reportProgrammingCommandError(error);
			const event = dispatch.mock.lastCall![0] as CustomEvent;
			expect(event.detail.kind).toBe("unexpected");
			expect(event.detail.message).toContain(error.message);
		}
	});
	it("keeps typed operator rejections brief and unexpected failures diagnostic", () => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		reportProgrammingCommandError(
			new ApiRequestError("Preset not Recorded", 404),
		);
		expect((dispatch.mock.calls[0][0] as CustomEvent).detail).toEqual({
			message: "Preset not Recorded",
			kind: "operator",
		});
		for (const error of [
			new ApiRequestError("database failed", 500),
			new ApiRequestError("session expired", 401),
			new ApiRequestError("Invalid server response", 400),
			new TypeError("Failed to fetch"),
		]) {
			reportProgrammingCommandError(error);
			const event = dispatch.mock.lastCall![0] as CustomEvent;
			expect(event.type).toBe(COMMAND_ERROR_EVENT);
			expect(event.detail.kind).toBe("unexpected");
			expect(event.detail.message).toContain(error.message);
		}
	});
	it("supports native and OSC validation feedback without hiding unknown faults", () => {
		for (const message of [
			"No Selection",
			"fixture number is invalid",
			"Invalid fixture number",
			"Invalid command syntax",
			"invalid data: Invalid fixture number",
		])
			expect(commandErrorFeedback(message).kind).toBe("operator");
		for (const message of [
			"Preset recording service task failed: database unavailable",
			"unrecognised runtime failure",
			"The command line could not be synchronized",
			"Invalid fixture runtime snapshot",
		])
			expect(commandErrorFeedback(message).kind).toBe("unexpected");
	});
	it("uses concise known labels while preserving the full history message", () => {
		expect(
			commandErrorSummary("invalid data: preset 99999 does not exist"),
		).toBe("Preset not Recorded");
		expect(
			commandErrorSummary("The command requires a non-empty selection"),
		).toBe("No Selection");
		const message = "preset 99999 does not exist";
		expect(commandErrorFeedback(message).message).toBe(message);
	});
});
