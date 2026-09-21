import { describe, expect, it, vi } from "vitest";
import {
	createHttpNativeHardwareBridge,
	describeDevice,
	type ExtensionRuntimeSnapshot,
	summarizeExtensions,
} from "./nativeBridge";

function snapshot(
	instances: ExtensionRuntimeSnapshot["instances"],
	extra: Partial<ExtensionRuntimeSnapshot> = {},
): ExtensionRuntimeSnapshot {
	return {
		configuration_diagnostic: null,
		packages: [{ id: "com.example.wing", name: "Example Wing" }],
		instances,
		instance_diagnostics: [],
		...extra,
	};
}

function instance(
	state: string,
	extra: Partial<ExtensionRuntimeSnapshot["instances"][number]> = {},
): ExtensionRuntimeSnapshot["instances"][number] {
	return {
		id: "wing-1",
		extension_id: "com.example.wing",
		capabilities: ["control_surface"],
		state,
		last_error: null,
		protocol_errors: 0,
		inbound_drops: 0,
		...extra,
	};
}

describe("native extension health summary", () => {
	it("reports a running instance by its package name", () => {
		const status = summarizeExtensions(snapshot([instance("running")]));
		expect(status).toEqual({
			state: "connected",
			name: "Example Wing",
			message: null,
		});
		expect(describeDevice(status)).toBe("Device connected · Example Wing");
	});

	it("reports no configured extension with the host diagnostic", () => {
		expect(summarizeExtensions(snapshot([]))).toEqual({
			state: "unavailable",
			message: "no native hardware extension is configured on this desk",
		});
		expect(
			summarizeExtensions(
				snapshot([], {
					configuration_diagnostic: "extensions.json is malformed",
				}),
			).message,
		).toBe("extensions.json is malformed");
	});

	it("ignores running telemetry and timecode extensions", () => {
		expect(
			summarizeExtensions(
				snapshot([
					instance("running", { capabilities: ["telemetry_source"] }),
					instance("running", { capabilities: ["timecode_source"] }),
				]),
			),
		).toEqual({
			state: "unavailable",
			message: "no native hardware extension is configured on this desk",
		});
	});

	it("reports restarting and terminal instances with their last error", () => {
		const restarting = summarizeExtensions(
			snapshot([
				instance("restarting { failures: 2, delay: 2s }", {
					last_error: "device unplugged",
				}),
			]),
		);
		expect(restarting.state).toBe("starting");
		expect(restarting.message).toBe("Example Wing: device unplugged");

		const terminal = summarizeExtensions(
			snapshot([instance("terminal { failures: 5 }", { protocol_errors: 3 })]),
		);
		expect(terminal).toEqual({
			state: "error",
			name: "Example Wing",
			message: "Example Wing: 3 protocol errors, 0 dropped inputs",
		});
	});

	it("flags a running device alongside a failed one as an error", () => {
		const status = summarizeExtensions(
			snapshot([
				instance("running"),
				instance("stopped", { id: "b", extension_id: "com.other" }),
			]),
		);
		expect(status.state).toBe("error");
		expect(status.name).toBe("Example Wing");
	});
});

describe("HTTP native hardware bridge", () => {
	it("reads health through a read-only session and closes it", async () => {
		const fetchImpl = vi.fn(async (url: string | URL | Request) => {
			const path = String(url);
			if (path.endsWith("/api/v2/sessions"))
				return Response.json({ session_id: "s1", token: "t1" });
			if (path.endsWith("/api/v2/extensions"))
				return Response.json(snapshot([instance("running")]));
			return new Response(null, { status: 204 });
		});
		const bridge = createHttpNativeHardwareBridge(fetchImpl as typeof fetch);
		await bridge.open({ host: "10.0.0.5", serverPort: 5000 });
		expect((await bridge.status()).state).toBe("connected");
		await bridge.close();

		const [create, read, close] = fetchImpl.mock.calls as unknown as Array<
			[string, RequestInit]
		>;
		expect(create[0]).toBe("http://10.0.0.5:5000/api/v2/sessions");
		expect(JSON.parse(String(create[1].body))).toEqual({ role: "visualizer" });
		expect(read[0]).toBe("http://10.0.0.5:5000/api/v2/extensions");
		expect(read[1].headers).toEqual({ authorization: "Bearer t1" });
		expect(close[0]).toBe("http://10.0.0.5:5000/api/v2/sessions/s1");
		expect(close[1].method).toBe("DELETE");
		expect((await bridge.status()).state).toBe("error");
	});

	it("turns refused and unreachable requests into actionable errors", async () => {
		const refused = createHttpNativeHardwareBridge(
			vi.fn(async () => new Response(null, { status: 401 })) as typeof fetch,
		);
		await expect(refused.open({ host: "h", serverPort: 1 })).rejects.toThrow(
			"the desk refused the request (is a desk token required?)",
		);

		let calls = 0;
		const unreachable = createHttpNativeHardwareBridge(
			vi.fn(async () => {
				calls += 1;
				if (calls === 1) return Response.json({ session_id: "s", token: "t" });
				throw new Error("Load failed");
			}) as typeof fetch,
		);
		await unreachable.open({ host: "h", serverPort: 1 });
		expect(await unreachable.status()).toEqual({
			state: "error",
			message: "desk unreachable at http://h:1: Load failed",
		});
	});
});
