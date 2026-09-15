/**
 * What the CAD title's add buttons open, inside the patch scope the library flow writes through.
 *
 * Trusses, stage elements and curtains are chosen from their own parts; any other Venue object still
 * comes from the fixture library, searched for its kind. Both stay mounted for the life of the CAD
 * screen, so the press that opens one is never mistaken for a press it already handled. A placed part
 * is announced to the CAD screen, which selects it and opens Info.
 */
import { FixtureAddFlow } from "@tosklight/patch";
import { CadAddPartModal } from "./CadAddPartModal";
import { type CadAddKind, useCadTools } from "./cadTools";

/** The library search each add button opens when it is the library that answers it. */
const CAD_ADD_PRESETS: Record<CadAddKind, { type: string; query: string }> = {
	truss: { type: "rigging", query: "Truss" },
	stage: { type: "venue", query: "Stage" },
	curtain: { type: "venue", query: "Curtain" },
	primitive: { type: "venue", query: "" },
	venue: { type: "", query: "" },
};

export function CadAddFlows({
	add,
	onError,
}: {
	/** The last add button pressed, and how many presses there have been. */
	add: { kind: CadAddKind; request: number };
	onError(reason: unknown): void;
}) {
	const tools = useCadTools();
	return (
		<>
			<CadAddPartModal
				kind={add.kind}
				request={add.request}
				onPlaced={tools.announcePlaced}
				onError={onError}
			/>
			<FixtureAddFlow
				scope="venue"
				addRequest={add.kind === "venue" ? add.request : 0}
				initialTypeFilter={CAD_ADD_PRESETS[add.kind].type}
				initialQuery={CAD_ADD_PRESETS[add.kind].query}
			/>
		</>
	);
}
