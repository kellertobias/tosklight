import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import type { RuntimeDiagnosticsSnapshot } from "../../api/generated/light-wire";
import type { OutputRoute, VersionedObject } from "../../api/types";
import {
	deskStateDiagnostic,
	type DeskStateDiagnostic,
} from "./deskStateDiagnostics";

type ReadDiagnostics = () => Promise<RuntimeDiagnosticsSnapshot>;

const OutputDiagnosticsContext = createContext<readonly DeskStateDiagnostic[]>([]);

export function DeskStateDiagnosticsProvider({
	children,
	enabled,
	readDiagnostics,
	outputRoutes,
	pollMilliseconds = 1_500,
}: PropsWithChildren<{
	enabled: boolean;
	readDiagnostics: ReadDiagnostics;
	outputRoutes: readonly VersionedObject<OutputRoute>[];
	pollMilliseconds?: number;
}>) {
	const [snapshot, setSnapshot] = useState<RuntimeDiagnosticsSnapshot | null>(null);
	const diagnostics = useMemo(
		() => (snapshot ? currentOutputDiagnostics(snapshot, outputRoutes) : []),
		[outputRoutes, snapshot],
	);

	useEffect(() => {
		if (!enabled) {
			setSnapshot(null);
			return;
		}
		let current = true;
		const refresh = async () => {
			try {
				const snapshot = await readDiagnostics();
				if (current) setSnapshot(snapshot);
			} catch {
				// Connection/authentication faults already belong to the shared shell status lane.
				if (current) setSnapshot(null);
			}
		};
		void refresh();
		const timer = globalThis.setInterval(() => void refresh(), pollMilliseconds);
		return () => {
			current = false;
			globalThis.clearInterval(timer);
		};
	}, [enabled, pollMilliseconds, readDiagnostics]);

	return (
		<OutputDiagnosticsContext.Provider value={diagnostics}>
			{children}
		</OutputDiagnosticsContext.Provider>
	);
}

export function useDeskStateDiagnostics(shellError: string | null) {
	const output = useContext(OutputDiagnosticsContext);
	return useMemo(
		() => (shellError ? [deskStateDiagnostic(shellError), ...output] : output),
		[shellError, output],
	);
}

interface OutputRouteDiagnostic {
	protocol: string;
	universe: number;
	destination: string;
	enabled: boolean;
}

export function currentOutputDiagnostics(
	snapshot: Pick<RuntimeDiagnosticsSnapshot, "output_routes">,
	outputRoutes: readonly VersionedObject<OutputRoute>[] = [],
): readonly DeskStateDiagnostic[] {
	const routes = decodeOutputRoutes(snapshot.output_routes).filter(
		(route) => route.enabled,
	);
	const grouped = new Map<string, OutputRouteDiagnostic[]>();
	for (const route of routes) {
		const key = `${route.protocol}\u0000${route.universe}\u0000${route.destination}`;
		const existing = grouped.get(key);
		if (existing) existing.push(route);
		else grouped.set(key, [route]);
	}
	const networkDuplicates = [...grouped.values()]
		.filter((matches) => matches.length > 1)
		.map(([route, ...duplicates]) => ({
			id: `duplicate-output-${slug(route.protocol)}-${route.universe}-${slug(route.destination)}`,
			title: `Duplicate output · ${protocolLabel(route.protocol)} universe ${route.universe} · ${route.destination}`,
			summary: `${duplicates.length + 1} enabled output routes send ${protocolLabel(route.protocol)} universe ${route.universe} to ${route.destination}. The same output target and address must be owned by one route.`,
			action: `Open Setup → Outputs and disable or remove the duplicate route for ${protocolLabel(route.protocol)} universe ${route.universe} at ${route.destination}.`,
		}));
	const usbClaims = new Map<string, OutputRoute[]>();
	for (const route of outputRoutes.map((entry) => entry.body)) {
		if (!route.enabled || route.target?.kind !== "usb_endpoint") continue;
		const claims = usbClaims.get(route.target.endpoint_id);
		if (claims) claims.push(route);
		else usbClaims.set(route.target.endpoint_id, [route]);
	}
	const usbDuplicates = [...usbClaims.entries()]
		.filter(([, claims]) => claims.length > 1)
		.map(([endpointId, claims]) => {
			const universes = claims
				.map((route) => route.logical_universe)
				.sort((left, right) => left - right)
				.join(", ");
			return {
				id: `duplicate-output-usb-${slug(endpointId)}`,
				title: `Duplicate output · USB DMX device · universes ${universes}`,
				summary: `The same USB DMX device is targeted by ${claims.length} enabled routes for logical universes ${universes}. One USB DMX device can output one logical universe, so the desk suppresses output instead of choosing one.`,
				action: "Open Setup → Outputs and disable or remove the extra device routes. Keep only the intended logical universe for this USB DMX device.",
			};
		});
	return [...networkDuplicates, ...usbDuplicates];
}

function decodeOutputRoutes(value: unknown): OutputRouteDiagnostic[] {
	if (!Array.isArray(value)) return [];
	return value.flatMap((candidate) => {
		if (!candidate || typeof candidate !== "object") return [];
		const route = candidate as Record<string, unknown>;
		if (
			typeof route.protocol !== "string" ||
			typeof route.universe !== "number" ||
			!Number.isSafeInteger(route.universe) ||
			typeof route.destination !== "string" ||
			typeof route.enabled !== "boolean"
		)
			return [];
		return [
			{
				protocol: route.protocol,
				universe: route.universe,
				destination: route.destination,
				enabled: route.enabled,
			},
		];
	});
}

function protocolLabel(protocol: string) {
	if (/art.?net/iu.test(protocol)) return "Art-Net";
	if (/s.?acn/iu.test(protocol)) return "sACN";
	return protocol;
}

function slug(value: string) {
	return value.toLocaleLowerCase().replace(/[^a-z0-9]+/gu, "-");
}
