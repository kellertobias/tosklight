/**
 * What a press of a CAD add button does.
 *
 * **Add truss**, **Add stage element**, **Add scenery** and **Add primitive** place a part at once:
 * the part a press names — chosen from the button's caret menu, which the button then remembers — or
 * else the part the button last placed. **Add venue element** opens the picture list of every Venue
 * profile, where an element is selected and added, or held with **Add Several** so every press on a
 * viewport places another copy. **Place Multiple**, the button beside a truss or stage element in
 * its part menu, opens that part's arrangement instead: a run of truss or a grid of stage
 * elements, placed in one go. This stays mounted for the life of
 * the CAD screen, so the press that opened or placed something is never mistaken for a new one.
 * What is placed is announced to the CAD screen, which selects it and opens Info.
 */
import { useEffect, useRef, useState } from "react";
import type { PlanPlacement } from "./bulkPlacement";
import { type BulkShape, CadBulkAddModal, type TrussSize } from "./CadBulkAddModal";
import { CadVenueElementModal } from "./CadVenueElementModal";
import { chosenPart, rememberPart } from "./cadAddChoice";
import { type FixtureLibrary, placedWith, placeProfile, readLibrary } from "./cadPlacement";
import { chooseAndImportModel, LOAD_MODEL } from "./cadModelImport";
import { type CadAddKind, useCadTools } from "./cadTools";
import { definitionForProfile, findPart, partKey, partLabel, type VenuePart } from "./venueParts";

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
	part: VenuePart;
	label: string;
	footprint?: { width: number; depth: number };
	truss?: TrussSize;
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
	// A model file being imported: long enough to show, and to say when it is done.
	const [importing, setImporting] = useState(false);

	async function loadModel() {
		try {
			const imported = await chooseAndImportModel(() => setImporting(true));
			if (imported) tools.announcePlaced([imported.fixtureId]);
		} catch (reason) {
			onError(`Could not load the 3D model: ${String(reason)}`);
		} finally {
			setImporting(false);
		}
	}
	const handled = useRef(add.request);

	async function place(
		profileId: string,
		label: string,
		known?: FixtureLibrary,
		placements?: readonly PlanPlacement[],
		part?: VenuePart,
	) {
		setPlacing(true);
		const result = await placeProfile(profileId, label, known, placements, placedWith(part));
		setPlacing(false);
		if (!result.ok) {
			onError(result.reason);
			return false;
		}
		tools.announcePlaced(result.fixtureIds);
		return true;
	}

	/** Opens a wizard, with the part's own footprint when this computer's library knows it. */
	async function openBulk(shape: BulkShape, part: VenuePart, label: string) {
		const library = await readLibrary();
		const scenery =
			library.state === "ready"
				? definitionForProfile(library.definitions, part.profileId)?.profile_snapshot?.scenery
				: undefined;
		const size = scenery?.default_size_metres;
		setBulk({
			shape,
			part,
			label,
			// A profile's y is up; a footprint is what it covers on the floor.
			footprint: size ? { width: size.x, depth: size.z } : undefined,
			// A truss's length is its width, which a straight section lets the operator set.
			truss:
				scenery && size
					? {
							metres: { x: size.x, y: size.y, z: size.z },
							lengthAdjustable: scenery.adjustable.width,
							minimumLength: scenery.minimum_size_metres.x,
							maximumLength: scenery.maximum_size_metres.x,
						}
					: undefined,
		});
	}

	useEffect(() => {
		if (add.request === handled.current) return;
		handled.current = add.request;
		if (add.kind === "primitive" && add.profileId === LOAD_MODEL) {
			void loadModel();
			return;
		}
		if (add.kind === "venue") {
			if (add.profileId) void place(add.profileId, "venue element");
			else setVenueOpen(true);
			return;
		}
		const named = add.profileId ? findPart(add.kind, add.profileId) : undefined;
		if (named) rememberPart(add.kind, partKey(named.part));
		const found = named ?? chosenPart(add.kind);
		if (add.several && (add.kind === "truss" || add.kind === "stage")) {
			void openBulk(add.kind, found.part, partLabel(found));
			return;
		}
		void place(found.part.profileId, partLabel(found), undefined, undefined, found.part);
	});

	if (bulk)
		return (
			<CadBulkAddModal
				shape={bulk.shape}
				partLabel={bulk.label}
				footprint={bulk.footprint}
				truss={bulk.truss}
				placing={placing}
				onClose={() => setBulk(null)}
				onPlace={(placements, size) =>
					void place(
						bulk.part.profileId,
						bulk.label,
						undefined,
						placements,
						size ? { ...bulk.part, sizeMetres: size } : bulk.part,
					).then((placed) => {
						if (placed) setBulk(null);
					})
				}
			/>
		);

	if (importing)
		return (
			<p className="cad-import-status" role="status">
				Loading the 3D model…
			</p>
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
			onAddSeveral={(profileId, name) => {
				setVenueOpen(false);
				tools.startPlacing({ profileId, name });
			}}
		/>
	) : null;
}
