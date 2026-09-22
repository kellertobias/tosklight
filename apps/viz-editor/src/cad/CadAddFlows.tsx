/**
 * What a press of a CAD add button does.
 *
 * **Add truss**, **Add stage element**, **Add curtain** and **Add primitive** place a part at once:
 * the part a press names — chosen from the button's caret menu, which the button then remembers — or
 * else the part the button last placed. **Add venue element** opens the picture list of every Venue
 * profile. **Place several…**, in the truss and stage part menus, opens that part's wizard instead:
 * a field of stage elements, or rows of truss, placed in one go. This stays mounted for the life of
 * the CAD screen, so the press that opened or placed something is never mistaken for a new one.
 * What is placed is announced to the CAD screen, which selects it and opens Info.
 */
import { useEffect, useRef, useState } from "react";
import type { PlanPlacement } from "./bulkPlacement";
import { type BulkShape, CadBulkAddModal } from "./CadBulkAddModal";
import { CadVenueElementModal } from "./CadVenueElementModal";
import { chosenPart, rememberPart } from "./cadAddChoice";
import { type FixtureLibrary, placeProfile, readLibrary } from "./cadPlacement";
import { type CadAddKind, useCadTools } from "./cadTools";
import { definitionForProfile, findPart, partLabel } from "./venueParts";

/** One press of an add button: what it adds, the part it names, and how many presses there have been. */
export interface CadAddRequest {
	kind: CadAddKind;
	profileId?: string;
	/** Whether the press asked for the part's wizard rather than one of the part. */
	several?: boolean;
	request: number;
}

/** The wizard that is open: which shape it is, and the part it places. */
interface BulkFlow {
	shape: BulkShape;
	profileId: string;
	label: string;
	footprint?: { width: number; depth: number };
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
	const [bulk, setBulk] = useState<BulkFlow | null>(null);
	const [placing, setPlacing] = useState(false);
	const handled = useRef(add.request);

	async function place(
		profileId: string,
		label: string,
		known?: FixtureLibrary,
		placements?: readonly PlanPlacement[],
	) {
		setPlacing(true);
		const result = await placeProfile(profileId, label, known, placements);
		setPlacing(false);
		if (!result.ok) {
			onError(result.reason);
			return false;
		}
		tools.announcePlaced(result.fixtureIds);
		return true;
	}

	/** Opens a wizard, with the part's own footprint when this computer's library knows it. */
	async function openBulk(shape: BulkShape, profileId: string, label: string) {
		const library = await readLibrary();
		const scenery =
			library.state === "ready"
				? definitionForProfile(library.definitions, profileId)?.profile_snapshot?.scenery
				: undefined;
		const size = scenery?.default_size_metres;
		setBulk({
			shape,
			profileId,
			label,
			// A profile's y is up; a footprint is what it covers on the floor.
			footprint: size ? { width: size.x, depth: size.z } : undefined,
		});
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
		if (add.several && (add.kind === "truss" || add.kind === "stage")) {
			void openBulk(add.kind, found.part.profileId, partLabel(found));
			return;
		}
		void place(found.part.profileId, partLabel(found));
	});

	if (bulk)
		return (
			<CadBulkAddModal
				shape={bulk.shape}
				partLabel={bulk.label}
				footprint={bulk.footprint}
				placing={placing}
				onClose={() => setBulk(null)}
				onPlace={(placements) =>
					void place(bulk.profileId, bulk.label, undefined, placements).then((placed) => {
						if (placed) setBulk(null);
					})
				}
			/>
		);

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
