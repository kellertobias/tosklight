import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { ColorIntentReport } from "../../api/client/attributeConfiguration";
import { useAttributeConfigurationActions } from "../attributeConfiguration/AttributeConfigurationActions";
import { acceptedHeads } from "./acceptedColorReport";

/** At most one report read per this interval, however often the refresh key changes. */
export const COLOR_REPORT_MIN_INTERVAL_MILLIS = 1_000;

/**
 * One batched accepted-frame colour report for a set of fixtures (TL-550).
 *
 * A single request covers every fixture (never one request per row). It is re-read when the
 * fixture set or `refreshKey` (for example the Programmer projection) changes, throttled to one
 * read per second. A report not read from an accepted output frame is discarded, so nothing is
 * shown at the legacy contract or before the colour has been output. Failures are passive: the
 * last accepted report stays, and no message is raised.
 */
export function useAcceptedColorReport(
	fixtureIds: readonly string[],
	options: { enabled: boolean; refreshKey?: unknown },
): ColorIntentReport | null {
	const actions = useAttributeConfigurationActions();
	const key = fixtureIds.join(",");
	const [state, setState] = useState<{ key: string; report: ColorIntentReport } | null>(null);
	const lastRead = useRef(0);
	const { enabled, refreshKey } = options;
	useEffect(() => {
		if (!enabled || !actions || !key) return;
		let current = true;
		const read = () => {
			lastRead.current = Date.now();
			actions.colorIntentReport(key.split(",")).then(
				(report) => {
					if (current && acceptedHeads(report))
						setState({ key, report });
				},
				() => undefined,
			);
		};
		const wait = lastRead.current + COLOR_REPORT_MIN_INTERVAL_MILLIS - Date.now();
		if (wait <= 0) {
			read();
			return () => {
				current = false;
			};
		}
		const timer = setTimeout(read, wait);
		return () => {
			current = false;
			clearTimeout(timer);
		};
	}, [actions, enabled, key, refreshKey]);
	return enabled && state?.key === key ? state.report : null;
}

// ---------------------------------------------------------------------------------------------
// Deliberate "show me the Color details" requests (Fixture Sheet triangle → Color modal)
// ---------------------------------------------------------------------------------------------

export interface ColorDetailsRequest {
	sequence: number;
	fixtureId: string;
}

let detailsRequest: ColorDetailsRequest | null = null;
const detailsListeners = new Set<() => void>();

export const colorDetailsRequests = {
	get: () => detailsRequest,
	subscribe(listener: () => void) {
		detailsListeners.add(listener);
		return () => detailsListeners.delete(listener);
	},
	/** Asks the Color Special Dialog to open expanded on its per-fixture detail area. */
	request(fixtureId: string) {
		detailsRequest = { sequence: (detailsRequest?.sequence ?? 0) + 1, fixtureId };
		for (const listener of detailsListeners) listener();
	},
	/** The Color dialog consumed the request. */
	clear() {
		if (!detailsRequest) return;
		detailsRequest = null;
		for (const listener of detailsListeners) listener();
	},
	/** Test seam. */
	reset() {
		detailsRequest = null;
	},
};

export function useColorDetailsRequest() {
	return useSyncExternalStore(
		colorDetailsRequests.subscribe,
		colorDetailsRequests.get,
		colorDetailsRequests.get,
	);
}
