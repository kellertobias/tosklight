import crypto from "node:crypto";
import { createSocket } from "node:dgram";
import { countPerformancePatch } from "./desktop-performance-patch.mjs";

/** An owned, payload-discarding receiver; counters stay bounded to the configured universes. */
export async function createPerformanceOutputSink() {
	const socket = createSocket("udp4");
	let failure = null;
	let closed = false;
	let active = false;
	let invalid = 0;
	let unexpected = 0;
	let counts = new Map();
	socket.on("error", (error) => {
		failure = error;
	});
	socket.on("message", (packet) => {
		if (!active) return;
		if (
			packet.length !== 530 ||
			packet.subarray(0, 8).toString("binary") !== "Art-Net\0" ||
			packet.readUInt16LE(8) !== 0x5000 ||
			packet.readUInt16BE(16) !== 512
		) {
			invalid++;
			return;
		}
		const universe = packet.readUInt16LE(14);
		if (!counts.has(universe)) {
			unexpected++;
			return;
		}
		counts.set(universe, counts.get(universe) + 1);
	});
	const close = async () => {
		if (closed) return;
		closed = true;
		active = false;
		await new Promise((resolve, reject) => {
			try {
				socket.close(resolve);
			} catch (error) {
				if (error.code === "ERR_SOCKET_DGRAM_NOT_RUNNING") resolve();
				else reject(error);
			}
		});
	};
	try {
		await new Promise((resolve, reject) => {
			const failed = (error) => reject(error);
			socket.once("error", failed);
			socket.bind(0, "127.0.0.1", () => {
				socket.off("error", failed);
				resolve();
			});
		});
	} catch (error) {
		await close();
		throw error;
	}
	return {
		destination: `127.0.0.1:${socket.address().port}`,
		start(universes) {
			if (closed || active || universes.length > 1024)
				throw new Error(
					"Invalid performance receiver lifecycle or universe count",
				);
			counts = new Map(universes.map((universe) => [universe, 0]));
			invalid = 0;
			unexpected = 0;
			active = true;
		},
		stop() {
			active = false;
			return this.snapshot();
		},
		snapshot() {
			if (failure) throw failure;
			return {
				protocol: "art_net",
				destination: this.destination,
				measurement_scope: "host-bounded measurement invocation",
				payload_slots: 512,
				packets_received: [...counts.values()].reduce(
					(total, count) => total + count,
					0,
				),
				invalid_datagrams: invalid,
				unexpected_universe_datagrams: unexpected,
				universes: [...counts].map(([universe, packets]) => ({
					universe,
					packets,
				})),
			};
		},
		close,
	};
}

const enabledRoutes = (snapshot) => {
	if (!Array.isArray(snapshot?.objects))
		throw new Error("Performance output route snapshot is unavailable");
	return snapshot.objects.filter((object) => object.body?.enabled);
};
const sameUniverses = (actual, expected) =>
	actual.length === expected.length &&
	[...actual]
		.sort((a, b) => a - b)
		.every((universe, index) => universe === expected[index]);

/** Only the isolated benchmark show's routes are changed through ordinary revision-checked intents. */
export async function configurePerformanceOutputRoutes(api, destination) {
	if (!/^127\.0\.0\.1:\d+$/u.test(destination))
		throw new Error(
			"Performance output must target its owned loopback receiver",
		);
	const patch = await api.request("GET", "/api/v2/patch");
	const universes = countPerformancePatch(patch).universes;
	if (
		!universes.length ||
		universes.length > 1024 ||
		universes.some((universe) => universe > 32767)
	)
		throw new Error(
			"Performance Patch has no bounded Art-Net universe workload",
		);
	const previous = await api.request("GET", "/api/v2/objects/route");
	for (const object of enabledRoutes(previous)) {
		if (!Number.isSafeInteger(object.revision))
			throw new Error("Performance route is missing its revision");
		await api.request("POST", "/api/v2/output-routes/actions", {
			request_id: crypto.randomUUID(),
			action: {
				type: "update",
				route_id: object.id,
				expected_revision: object.revision,
				patch: { enabled: false },
			},
		});
	}
	for (const universe of universes)
		await api.request("POST", "/api/v2/output-routes/actions", {
			request_id: crypto.randomUUID(),
			action: {
				type: "create",
				route_id: `performance-${crypto.randomUUID()}`,
				route: {
					target: { kind: "network" },
					protocol: "art_net",
					logical_universe: universe,
					destination_universe: universe,
					delivery_mode: "unicast",
					destination,
					enabled: true,
					minimum_slots: 512,
				},
			},
		});
	const actual = enabledRoutes(
		await api.request("GET", "/api/v2/objects/route"),
	);
	if (
		!sameUniverses(
			actual.map((object) => object.body.logical_universe),
			universes,
		) ||
		actual.some(
			({ body }) =>
				body.protocol !== "art_net" ||
				body.delivery_mode !== "unicast" ||
				body.destination !== destination ||
				body.destination_universe !== body.logical_universe ||
				body.minimum_slots !== 512 ||
				(body.target?.kind ?? "network") !== "network",
		)
	)
		throw new Error(
			"Performance output routes do not cover the actual Patch exactly",
		);
	const diagnostics = await api.request("GET", "/api/v2/diagnostics");
	if (!Array.isArray(diagnostics?.output_routes))
		throw new Error("Performance route diagnostics are unavailable");
	const diagnosticRoutes = diagnostics.output_routes.filter(
		(route) => route.enabled,
	);
	if (
		!sameUniverses(
			diagnosticRoutes.map((route) => route.universe),
			universes,
		) ||
		diagnosticRoutes.some(
			(route) =>
				route.protocol !== "art_net" ||
				route.delivery_mode !== "unicast" ||
				route.destination !== destination,
		)
	)
		throw new Error(
			"Live performance output route diagnostics disagree with the patched universes",
		);
	return {
		routed_patch_universes: universes,
		output_route_scope: "all-patched-universes-loopback-artnet",
		loopback_output_destination: destination,
	};
}

/** A configured route is insufficient: every expected universe must deliver valid DMX. */
export function validatePerformanceOutputReceipt(receipt, expectedUniverses) {
	const validCount = (value) => Number.isSafeInteger(value) && value >= 0;
	if (
		!Array.isArray(expectedUniverses) ||
		!expectedUniverses.length ||
		expectedUniverses.length > 1024 ||
		new Set(expectedUniverses).size !== expectedUniverses.length ||
		expectedUniverses.some(
			(universe) => !validCount(universe) || universe > 32767,
		)
	)
		throw new Error("Invalid expected performance output universe coverage");
	if (
		receipt?.protocol !== "art_net" ||
		receipt.payload_slots !== 512 ||
		!validCount(receipt.packets_received) ||
		receipt.packets_received === 0 ||
		receipt.invalid_datagrams !== 0 ||
		receipt.unexpected_universe_datagrams !== 0
	)
		throw new Error(
			"Performance output receipt has no valid complete Art-Net delivery",
		);
	if (
		!Array.isArray(receipt.universes) ||
		!sameUniverses(
			receipt.universes.map((row) => row?.universe),
			[...expectedUniverses].sort((a, b) => a - b),
		) ||
		receipt.universes.some(
			(row) => !validCount(row.packets) || row.packets === 0,
		)
	)
		throw new Error(
			"Performance output receipt is missing complete per-universe delivery",
		);
	const counted = receipt.universes.reduce(
		(total, row) => total + row.packets,
		0,
	);
	if (!Number.isSafeInteger(counted) || counted !== receipt.packets_received)
		throw new Error("Performance output receipt packet totals disagree");
	return receipt;
}
