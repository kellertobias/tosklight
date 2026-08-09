import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiDriver, type CommandLineState, type Session } from "./api";

describe("ApiDriver command-line replacement", () => {
	afterEach(() => vi.unstubAllGlobals());

	it("rebases one command-line revision race against fresh authority", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValueOnce(commandLineResponse(1, "FIXTURE"))
			.mockResolvedValueOnce(
				new Response(
					"command-line revision conflict: expected revision 1, actual revision 2",
					{ status: 409 },
				),
			)
			.mockResolvedValueOnce(commandLineResponse(2, "GROUP"))
			.mockResolvedValueOnce(commandLineResponse(3, "RECORD CUE 2"));
		vi.stubGlobal("fetch", fetch);
		const driver = new ApiDriver("http://desk.local");
		driver.session = session();

		await expect(driver.setCommandLineText("RECORD CUE 2")).resolves.toMatchObject(
			{ commandLine: { revision: 3, text: "RECORD CUE 2" } },
		);
		expect(fetch).toHaveBeenCalledTimes(4);
		const firstPut = fetch.mock.calls[1]?.[1];
		const secondPut = fetch.mock.calls[3]?.[1];
		expect(new Headers(firstPut?.headers).get("if-match")).toBe("1");
		expect(new Headers(secondPut?.headers).get("if-match")).toBe("2");
		expect(JSON.parse(String(secondPut?.body))).toEqual({ text: "RECORD CUE 2" });
	});
});

function commandLineResponse(revision: number, text: string) {
	const commandLine: CommandLineState = {
		text,
		target: "FIXTURE",
		pristine: text.length === 0,
		revision,
		pending_choice: null,
	};
	return new Response(JSON.stringify(commandLine), {
		status: 200,
		headers: {
			"content-type": "application/json",
			etag: `"${revision}"`,
		},
	});
}

function session(): Session {
	return {
		session_id: "session",
		client_id: "client",
		token: "token",
		user: {
			id: "11111111-1111-4111-8111-111111111111",
			name: "Operator",
		},
		desk: {
			id: "22222222-2222-4222-8222-222222222222",
			osc_alias: "main",
		},
	};
}
