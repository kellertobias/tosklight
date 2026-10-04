import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import type {
	OutputOwnerReadout,
	OutputReadoutSnapshot,
} from "../../api/familyEncoderModels";
import type {
	DisplayedSource,
	DisplayedSourceLane,
} from "../programmerValues/displayedSource";
import {
	boundedFamilyFixtureIds,
	useFamilyEncodersContext,
} from "./FamilyEncodersProvider";

/**
 * Typed displayed-source readouts of one surface's fixtures (TL-594 consumer).
 *
 * - One HTTP read on claim gives the surface an immediate source and lease. On the Normal lane a
 *   stream readout claim then delivers each new publication with the Normal lane's own lease
 *   (merged with every other consumer and bounded to 512 owners by the session). Preload has no
 *   stream readouts; it reads its own Pending lineage over HTTP and never falls back to Live.
 * - Every delivery is observed by the provider's shared `DisplayedSourceReadouts`, so gesture
 *   sessions name the newest lease the operator was shown (`displayedSource(lane)`). Each
 *   consumer installs its own deliveries by its own lease order: another consumer's newer read
 *   never hides this consumer's readouts, and `displayedSource()` names the newest lease whose
 *   delivery covered this consumer's fixtures.
 * - Nothing is claimed while `enabled` is false, the selection is empty, or the document is
 *   hidden; the claim is released on unmount, hide and selection change.
 */
export interface FamilyReadouts {
	snapshot: OutputReadoutSnapshot | null;
	owner(fixtureId: string): OutputOwnerReadout | null;
	/** The lease a gesture on `lane` names on its edits, or null (latest-accepted adoption). */
	displayedSource(): DisplayedSource | null;
	/** After a `displayed_source_unavailable` hold: forget the lease and read again. */
	reread(): void;
}

function subscribeVisibility(onChange: () => void) {
	if (typeof document === "undefined") return () => undefined;
	document.addEventListener("visibilitychange", onChange);
	return () => document.removeEventListener("visibilitychange", onChange);
}

function documentVisible() {
	return typeof document === "undefined" || document.visibilityState !== "hidden";
}

export function useDocumentVisible() {
	return useSyncExternalStore(subscribeVisibility, documentVisible, () => true);
}

/** An out-of-order delivery older than the one this consumer already shows. */
function olderLease(
	next: OutputReadoutSnapshot,
	shown: OutputReadoutSnapshot | null,
) {
	return next.lease != null && shown?.lease != null && shown.lease > next.lease;
}

/** Re-reads of an unavailable Preload readout, one per interval (10 s in all). */
export const PRELOAD_UNAVAILABLE_RETRIES = 40;
export const PRELOAD_UNAVAILABLE_RETRY_MILLIS = 250;

export function useFamilyReadouts(
	lane: DisplayedSourceLane,
	fixtureIds: readonly string[],
	options: { enabled?: boolean; consumerId?: string } = {},
): FamilyReadouts {
	const enabled = options.enabled ?? true;
	const consumerId = options.consumerId ?? "family-readouts";
	const context = useFamilyEncodersContext();
	const readouts = context?.readouts ?? null;
	const session = context?.session ?? null;
	const visible = useDocumentVisible();
	const key = boundedFamilyFixtureIds(fixtureIds).join(",");
	const [snapshot, setSnapshot] = useState<OutputReadoutSnapshot | null>(null);
	const [generation, setGeneration] = useState(0);
	useEffect(() => {
		if (!enabled || !visible || !key || !readouts) {
			setSnapshot(null);
			return;
		}
		const ids = key.split(",");
		let current = true;
		const install = (next: OutputReadoutSnapshot) => {
			if (!current || next.lane !== lane) return;
			readouts.observe(next);
			setSnapshot((shown) => (olderLease(next, shown) ? shown : next));
		};
		// Preload has no stream: its Pending lineage publishes shortly after Preload is armed or
		// changed, so an early "no accepted Preload" is read again until it is available.
		let retries = PRELOAD_UNAVAILABLE_RETRIES;
		let timer: ReturnType<typeof setTimeout> | undefined;
		const retry = () => {
			if (!current || lane !== "preload" || retries <= 0) return;
			retries -= 1;
			timer = setTimeout(read, PRELOAD_UNAVAILABLE_RETRY_MILLIS);
		};
		const read = () => {
			readouts.read(lane, ids).then(
				(next) => {
					install(next);
					if (next.unavailable) retry();
				},
				() => retry(),
			);
		};
		read();
		const release =
			lane === "normal"
				? (session?.claimReadouts(ids, install, consumerId) ?? null)
				: null;
		return () => {
			current = false;
			clearTimeout(timer);
			release?.();
		};
	}, [consumerId, enabled, generation, key, lane, readouts, session, visible]);
	const owner = useCallback(
		(fixtureId: string) =>
			snapshot?.owners.find((entry) => entry.fixture_id === fixtureId) ?? null,
		[snapshot],
	);
	const displayedSource = useCallback(
		() => readouts?.displayedSource(lane, key ? key.split(",") : []) ?? null,
		[key, lane, readouts],
	);
	const reread = useCallback(() => {
		readouts?.forget(lane);
		setGeneration((value) => value + 1);
	}, [lane, readouts]);
	return { snapshot, owner, displayedSource, reread };
}
