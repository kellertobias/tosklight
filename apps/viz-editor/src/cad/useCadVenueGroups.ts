/**
 * The open show's Venue element groups, followed across windows and saved on every change.
 *
 * A change shows at once and is taken back, with the reason, when the show refuses it.
 */
import { useEffect, useRef, useState } from "react";
import type { CadEntity } from "./types";
import {
	EMPTY_VENUE_GROUPS,
	type VenueGroups,
	venueGroupAction,
	venueGroupsSession,
} from "./venueGroups";

export function useCadVenueGroups(documentKey: string | null) {
	const [groups, setGroups] = useState<VenueGroups>(EMPTY_VENUE_GROUPS);
	const [error, setError] = useState<string | null>(null);
	const current = useRef(groups);
	current.current = groups;

	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		venueGroupsSession
			.get()
			.then((loaded) => !disposed && setGroups(loaded ?? EMPTY_VENUE_GROUPS))
			.catch(() => !disposed && setGroups(EMPTY_VENUE_GROUPS));
		venueGroupsSession
			.onDelta((next) => !disposed && setGroups(next))
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => undefined);
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [documentKey]);

	function change(next: VenueGroups) {
		const previous = current.current;
		setGroups(next);
		setError(null);
		venueGroupsSession.save(next).catch((reason) => {
			setGroups(previous);
			setError(String(reason));
		});
	}

	/** Group or ungroup the selection, when that has something to do. */
	function run(
		action: "group" | "ungroup",
		entities: readonly CadEntity[],
		selectedIds: readonly string[],
	) {
		const next = venueGroupAction(current.current, entities, selectedIds, action);
		if (next) change(next);
	}

	return { groups, change, run, error };
}
