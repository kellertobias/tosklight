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
	it("keeps typed operator rejections brief and unexpected failures diagnostic", () => {
		const dispatch = vi.spyOn(window, "dispatchEvent");
		reportProgrammingCommandError(new ApiRequestError("Preset not Recorded", 404));
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
		]) expect(commandErrorFeedback(message).kind).toBe("operator");
		for (const message of [
			"Preset recording service task failed: database unavailable",
			"unrecognised runtime failure",
			"The command line could not be synchronized",
			"Invalid fixture runtime snapshot",
		]) expect(commandErrorFeedback(message).kind).toBe("unexpected");
	});
	it("uses concise known labels while preserving the full history message", () => {
		expect(commandErrorSummary("invalid data: preset 99999 does not exist"))
			.toBe("Preset not Recorded");
		expect(commandErrorSummary("The command requires a non-empty selection"))
			.toBe("No Selection");
		const message = "preset 99999 does not exist";
		expect(commandErrorFeedback(message).message).toBe(message);
	});
});
