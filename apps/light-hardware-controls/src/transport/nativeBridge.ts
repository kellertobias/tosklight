import type { DeviceStatus } from "../controller/types";

/**
 * Native Hardware mode observes the desk's supervised native-extension host
 * (docs/engineering/native-extension-sdk.md). The attached device is driven by an
 * extension package owned by the desk; its inputs reach the desk directly, so this
 * application never relays them. It only reads the host's health through
 * `GET /api/v2/extensions` with a read-only visualizer session.
 */
export interface NativeHardwareTarget {
	host: string;
	serverPort: number;
}

export interface NativeHardwareBridge {
	open(target: NativeHardwareTarget): Promise<void>;
	status(): Promise<DeviceStatus>;
	close(): Promise<void>;
}

/** Subset of `light_wire::v2::ExtensionRuntimeSnapshot` this application reads. */
export interface ExtensionRuntimeSnapshot {
	configuration_diagnostic: string | null;
	packages: Array<{ id: string | null; name: string | null }>;
	instances: Array<{
		id: string;
		extension_id: string;
		state: string;
		last_error: string | null;
		protocol_errors: number;
		inbound_drops: number;
	}>;
	instance_diagnostics: Array<{ instance_id: string; detail: string }>;
}

interface SessionResponse {
	session_id: string;
	token: string;
}

type Fetch = typeof fetch;

export function createHttpNativeHardwareBridge(
	fetchImpl: Fetch = (...arguments_) => fetch(...arguments_),
): NativeHardwareBridge {
	let baseUrl = "";
	let session: SessionResponse | null = null;

	const describeFailure = (response: Response) =>
		response.status === 401
			? "the desk refused the request (is a desk token required?)"
			: `the desk answered HTTP ${response.status}`;

	const bridge: NativeHardwareBridge = {
		async open({ host, serverPort }) {
			await bridge.close();
			baseUrl = `http://${host}:${serverPort}`;
			const response = await fetchImpl(`${baseUrl}/api/v2/sessions`, {
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ role: "visualizer" }),
			});
			if (!response.ok) throw new Error(describeFailure(response));
			session = (await response.json()) as SessionResponse;
		},

		async status() {
			if (!session)
				return { state: "error", message: "not connected to the desk" };
			let response: Response;
			try {
				response = await fetchImpl(`${baseUrl}/api/v2/extensions`, {
					headers: { authorization: `Bearer ${session.token}` },
				});
			} catch (error) {
				return {
					state: "error",
					message: `desk unreachable at ${baseUrl}: ${errorText(error)}`,
				};
			}
			if (!response.ok)
				return { state: "error", message: describeFailure(response) };
			return summarizeExtensions(
				(await response.json()) as ExtensionRuntimeSnapshot,
			);
		},

		async close() {
			const closing = session;
			session = null;
			if (!closing) return;
			await fetchImpl(`${baseUrl}/api/v2/sessions/${closing.session_id}`, {
				method: "DELETE",
				keepalive: true,
				headers: { authorization: `Bearer ${closing.token}` },
			}).catch(() => undefined);
		},
	};
	return bridge;
}

/** Used where no desktop runtime exists, such as the browser test bench. */
export const unavailableNativeHardwareBridge: NativeHardwareBridge = {
	open: async () => undefined,
	status: async () => ({
		state: "unavailable",
		message:
			"Native Hardware needs the ToskLight Hardware Controls desktop app",
	}),
	close: async () => undefined,
};

export function summarizeExtensions(
	snapshot: ExtensionRuntimeSnapshot,
): DeviceStatus {
	const names = new Map(
		snapshot.packages.flatMap((entry) =>
			entry.id ? [[entry.id, entry.name ?? entry.id] as const] : [],
		),
	);
	const label = (extensionId: string) => names.get(extensionId) ?? extensionId;
	const instances = snapshot.instances;
	if (instances.length === 0) {
		const detail =
			snapshot.configuration_diagnostic ??
			snapshot.instance_diagnostics[0]?.detail ??
			"no native hardware extension is configured on this desk";
		return { state: "unavailable", message: detail };
	}
	const running = instances.filter((entry) => entry.state === "running");
	const waiting = instances.filter((entry) =>
		/^(starting|handshaking|restarting)/u.test(entry.state),
	);
	const failed = instances.filter(
		(entry) => !running.includes(entry) && !waiting.includes(entry),
	);
	const problems = [
		...instances.flatMap((entry) =>
			entry.last_error
				? [`${label(entry.extension_id)}: ${entry.last_error}`]
				: [],
		),
		...instances.flatMap((entry) =>
			entry.protocol_errors + entry.inbound_drops > 0
				? [
						`${label(entry.extension_id)}: ${entry.protocol_errors} protocol errors, ${entry.inbound_drops} dropped inputs`,
					]
				: [],
		),
	];
	const message = problems.length > 0 ? problems.join("; ") : null;
	if (running.length > 0)
		return {
			state: failed.length > 0 ? "error" : "connected",
			name: running.map((entry) => label(entry.extension_id)).join(", "),
			message,
		};
	if (waiting.length > 0)
		return {
			state: "starting",
			name: waiting.map((entry) => label(entry.extension_id)).join(", "),
			message: message ?? waiting[0].state,
		};
	return {
		state: "error",
		name: failed.map((entry) => label(entry.extension_id)).join(", "),
		message: message ?? failed.map((entry) => entry.state).join(", "),
	};
}

export function describeDevice(status: DeviceStatus): string {
	const detail = status.message ? ` · ${status.message}` : "";
	const name = status.name ? ` · ${status.name}` : "";
	switch (status.state) {
		case "connected":
			return `Device connected${name}${detail}`;
		case "starting":
			return `Device starting${name}${detail}`;
		case "unavailable":
			return `No device available${detail}`;
		case "error":
			return `Device error${name}${detail}`;
		case "stopped":
			return "Native Hardware inactive";
	}
}

function errorText(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}
