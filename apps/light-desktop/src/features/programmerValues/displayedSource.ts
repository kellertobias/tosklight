import type {
	OutputReadoutSnapshot,
	VisualizationLane,
} from "../../api/generated/light-wire";
import { WireValidationError } from "../../api/wireValidation";

/**
 * Displayed-source readouts (TL-594).
 *
 * A surface reads typed readouts of one accepted output source from
 * `GET /api/v2/output/readouts`. The server leases exactly the source it delivered to this
 * session; the surface keeps the newest lease per lane and names it on the first Angle edit
 * of a gesture (`displayed_source`), so the server adopts precisely what the operator saw.
 *
 * - Reads are on demand only. Nothing here polls, so hidden surfaces cost nothing, and
 *   concurrent reads of the same lane and owners share one request.
 * - Out-of-order responses never replace a newer lease with an older one, and they never
 *   discard it either: per lane the client keeps the leases of the last
 *   {@link RECENT_DISPLAYED_SOURCES} delivered sources (the server's ring), so a second
 *   consumer's read never takes away the lease of what another consumer is showing.
 * - The server leases one accepted source once per session lane, so every reader of the same
 *   frame receives the same lease. `displayedSource(lane, fixtureIds)` names the newest lease
 *   whose delivery covered those fixtures (a Pending capture holds an edit for a member it
 *   never delivered); without fixture ids it names the newest lease.
 * - A gesture reads its lease once at start; later reads by any consumer never change it.
 * - When the server answers an edit with the `displayed_source_unavailable` hold, the lease
 *   is gone: `forget` it and re-read. Nothing is retried with latest on the client either.
 *
 * Edits sent without a displayed source (OSC, HTTP integrators) keep the server's
 * latest-accepted adoption.
 */

export type DisplayedSourceLane = VisualizationLane;

export interface DisplayedSource {
	lane: DisplayedSourceLane;
	lease: number;
}

export type ReadoutRequest = (
	path: string,
	init: { headers?: Record<string, string> },
) => Promise<unknown>;

export interface DisplayedSourceReadoutsOptions {
	request: ReadoutRequest;
	/** The loaded show; sent as the `X-Tosk-Show` guard when known. */
	showId?: () => string | null | undefined;
}

export function outputReadoutsPath(
	lane: DisplayedSourceLane,
	fixtureIds: readonly string[],
) {
	const owners = fixtureIds.map(encodeURIComponent).join(",");
	return `/api/v2/output/readouts?lane=${lane}&fixture_ids=${owners}`;
}

function record(value: unknown, path: string): Record<string, unknown> {
	if (typeof value !== "object" || value === null || Array.isArray(value))
		throw new WireValidationError(path, "object", value);
	return value as Record<string, unknown>;
}

/** Validates the fields the client relies on; unknown fields are tolerated. */
export function decodeOutputReadoutSnapshot(
	value: unknown,
): OutputReadoutSnapshot {
	const snapshot = record(value, "$");
	if (snapshot.lane !== "normal" && snapshot.lane !== "preload")
		throw new WireValidationError("$.lane", "normal | preload", snapshot.lane);
	const lease = snapshot.lease ?? null;
	if (lease !== null && (typeof lease !== "number" || !Number.isSafeInteger(lease)))
		throw new WireValidationError("$.lease", "integer | null", lease);
	if (!Array.isArray(snapshot.owners))
		throw new WireValidationError("$.owners", "array", snapshot.owners);
	snapshot.owners.forEach((owner, index) => {
		const entry = record(owner, `$.owners[${index}]`);
		if (typeof entry.fixture_id !== "string")
			throw new WireValidationError(
				`$.owners[${index}].fixture_id`,
				"string",
				entry.fixture_id,
			);
		const position = record(entry.position, `$.owners[${index}].position`);
		if (!Array.isArray(position.commands))
			throw new WireValidationError(
				`$.owners[${index}].position.commands`,
				"array",
				position.commands,
			);
	});
	return snapshot as unknown as OutputReadoutSnapshot;
}

/** Leases kept per lane: the server retains the newest 8 delivered sources per session lane. */
export const RECENT_DISPLAYED_SOURCES = 8;

interface DeliveredSource {
	lease: number;
	snapshot: OutputReadoutSnapshot;
	/** Every fixture any consumer was shown from this source. */
	owners: Set<string>;
}

export class DisplayedSourceReadouts {
	/** Newest first, distinct leases, at most {@link RECENT_DISPLAYED_SOURCES} per lane. */
	private readonly recentByLane = new Map<DisplayedSourceLane, DeliveredSource[]>();
	private readonly inFlight = new Map<string, Promise<OutputReadoutSnapshot>>();

	constructor(private readonly options: DisplayedSourceReadoutsOptions) {}

	/** Fetches readouts of `fixtureIds` and keeps the delivered lease. */
	read(
		lane: DisplayedSourceLane,
		fixtureIds: readonly string[],
	): Promise<OutputReadoutSnapshot> {
		const path = outputReadoutsPath(lane, fixtureIds);
		const pending = this.inFlight.get(path);
		if (pending) return pending;
		const showId = this.options.showId?.();
		const request = this.options
			.request(path, showId ? { headers: { "X-Tosk-Show": showId } } : {})
			.then((value) => {
				const snapshot = decodeOutputReadoutSnapshot(value);
				this.observe(snapshot);
				return snapshot;
			})
			.finally(() => this.inFlight.delete(path));
		this.inFlight.set(path, request);
		return request;
	}

	/**
	 * Records a leased snapshot (also for WebSocket `readouts` messages). An older lease is
	 * kept beside newer ones; a repeated lease merges the fixtures it was delivered for.
	 * An unavailable snapshot clears the lane: there is no displayed source to name.
	 * Returns whether the snapshot became the lane's current (newest) one.
	 */
	observe(snapshot: OutputReadoutSnapshot) {
		const lease = snapshot.lease ?? null;
		if (lease === null) {
			this.recentByLane.delete(snapshot.lane);
			return true;
		}
		const recent = (this.recentByLane.get(snapshot.lane) ?? []).slice();
		const index = recent.findIndex((entry) => entry.lease === lease);
		const owners = new Set(index >= 0 ? recent[index].owners : []);
		for (const owner of snapshot.owners) owners.add(owner.fixture_id);
		if (index >= 0) recent.splice(index, 1);
		const position = recent.findIndex((entry) => entry.lease < lease);
		const at = position < 0 ? recent.length : position;
		recent.splice(at, 0, { lease, snapshot, owners });
		this.recentByLane.set(
			snapshot.lane,
			recent.slice(0, RECENT_DISPLAYED_SOURCES),
		);
		return at === 0;
	}

	latest(lane: DisplayedSourceLane): OutputReadoutSnapshot | null {
		return this.recentByLane.get(lane)?.[0]?.snapshot ?? null;
	}

	/**
	 * The source a writer attaches to the first edit of a gesture on `lane`: the newest lease
	 * whose delivery covered every one of `fixtureIds`, else the newest lease.
	 */
	displayedSource(
		lane: DisplayedSourceLane,
		fixtureIds: readonly string[] = [],
	): DisplayedSource | null {
		const recent = this.recentByLane.get(lane) ?? [];
		const covering = recent.find((entry) =>
			fixtureIds.every((fixtureId) => entry.owners.has(fixtureId)),
		);
		const lease = (covering ?? recent[0])?.lease;
		return lease == null ? null : { lane, lease };
	}

	/**
	 * After a `displayed_source_unavailable` hold the named lease is gone: forget it (or the
	 * whole lane without a lease) and re-read before reuse. A gesture already in progress keeps
	 * the lease it read at its start.
	 */
	forget(lane: DisplayedSourceLane, lease?: number) {
		if (lease === undefined) {
			this.recentByLane.delete(lane);
			return;
		}
		const recent = this.recentByLane.get(lane)?.filter((entry) => entry.lease !== lease);
		if (recent?.length) this.recentByLane.set(lane, recent);
		else this.recentByLane.delete(lane);
	}
}
