/**
 * What a press of a CAD add button does.
 *
 * **Add truss**, **Add stage element**, **Add curtain** and **Add primitive** place a part at once:
 * the part a press names — chosen from the button's caret menu, which the button then remembers — or
 * else the part the button last placed. **Add venue element** opens the picture list of every Venue
 * profile. This stays mounted for the life of the CAD screen, so the press that opened or placed
 * something is never mistaken for a new one. A placed object is announced to the CAD screen, which
 * selects it and opens Info.
 */
import { useEffect, useRef, useState } from "react";
import { CadVenueElementModal } from "./CadVenueElementModal";
import { chosenPart, rememberPart } from "./cadAddChoice";
import { type FixtureLibrary, placeProfile } from "./cadPlacement";
import { type CadAddKind, useCadTools } from "./cadTools";
import { findPart, partLabel } from "./venueParts";

/** One press of an add button: what it adds, the part it names, and how many presses there have been. */
export interface CadAddRequest {
	kind: CadAddKind;
	profileId?: string;
	request: number;
}

export function CadAddFlows({
	add,
	onError,
}: {
	add: CadAddRequest;
	onError(reason: unknown): void;
}) {
	const tools = useCadTools();
	const [venueOpen, setVenueOpen] = useState(false);
	const [placing, setPlacing] = useState(false);
	const handled = useRef(add.request);

	async function place(profileId: string, label: string, known?: FixtureLibrary) {
		setPlacing(true);
		const result = await placeProfile(profileId, label, known);
		setPlacing(false);
		if (!result.ok) {
			onError(result.reason);
			return false;
		}
		tools.announcePlaced(result.fixtureId);
		return true;
	}

	useEffect(() => {
		if (add.request === handled.current) return;
		handled.current = add.request;
		if (add.kind === "venue") {
			if (add.profileId) void place(add.profileId, "venue element");
			else setVenueOpen(true);
			return;
		}
		const named = add.profileId ? findPart(add.kind, add.profileId) : undefined;
		if (named) rememberPart(add.kind, named.part.profileId);
		const found = named ?? chosenPart(add.kind);
		void place(found.part.profileId, partLabel(found));
	});

	return venueOpen ? (
		<CadVenueElementModal
			placing={placing}
			onClose={() => setVenueOpen(false)}
			onChoose={(profileId, name, library) =>
				void place(profileId, name, library).then((placed) => {
					if (placed) setVenueOpen(false);
				})
			}
		/>
	) : null;
}
