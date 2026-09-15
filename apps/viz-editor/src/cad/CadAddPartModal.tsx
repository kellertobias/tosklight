/**
 * The dialog behind **Add truss** and **Add stage element**, and the direct add behind **Add curtain**.
 *
 * Choosing takes two steps: the truss section or what a stage element stands on, then the part or
 * platform size, each shown by its picture on a dark ground. The chosen part is placed at the stage
 * origin like any other Venue object, and the CAD screen selects it so Info opens to place and size
 * it. A step with a single choice is skipped.
 */
import {
	type FixtureDefinition,
	newPatchFixtureCandidate,
	usePatch,
} from "@tosklight/patch";
import { Button, ModalFrame } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import {
	definitionForProfile,
	nextVirtualNumber,
	PARAMETRIC_CURTAIN_PROFILE_ID,
	previewOf,
	STAGE_TYPES,
	TRUSS_TYPES,
	type VenuePart,
	type VenuePartGroup,
} from "./venueParts";
import "./cadAddParts.css";

/** What the add buttons ask for; `venue` is any other object and is left to the fixture library. */
export type CadPartKind = "truss" | "stage" | "curtain" | "venue";

type ChosenKind = "truss" | "stage";

const TITLES: Record<ChosenKind, { title: string; groupsLabel: string }> = {
	truss: { title: "Add truss", groupsLabel: "Truss type" },
	stage: { title: "Add stage element", groupsLabel: "Feet" },
};

function PartTile({
	label,
	detail,
	definition,
	onChoose,
}: {
	label: string;
	detail?: string;
	definition: FixtureDefinition | undefined;
	onChoose(): void;
}) {
	const preview = previewOf(definition);
	return (
		<button
			type="button"
			className="cad-part-tile"
			disabled={!definition}
			onClick={onChoose}
		>
			<span className="cad-part-preview">
				{preview ? <img src={preview} alt="" /> : <span aria-hidden="true">No picture</span>}
			</span>
			<strong>{label}</strong>
			<small>{definition ? (detail ?? definition.name) : "Not in this library"}</small>
		</button>
	);
}

export function CadAddPartModal({
	kind,
	request,
	definitions,
	onPlaced,
	onError,
}: {
	kind: CadPartKind;
	/** Bumped on every press of an add button; each press opens or places once. */
	request: number;
	definitions: readonly FixtureDefinition[];
	onPlaced(fixtureId: string): void;
	onError(reason: unknown): void;
}) {
	const patch = usePatch();
	const [open, setOpen] = useState<ChosenKind | null>(null);
	const [group, setGroup] = useState<VenuePartGroup | null>(null);
	const [placing, setPlacing] = useState(false);
	const handled = useRef(request);

	async function place(part: Pick<VenuePart, "profileId" | "label">) {
		const definition = definitionForProfile(definitions, part.profileId);
		if (!definition) {
			onError(`${part.label} is not in this machine's fixture library.`);
			return;
		}
		setPlacing(true);
		try {
			const candidate = newPatchFixtureCandidate({
				name: definition.name,
				fixture_number: null,
				virtual_fixture_number: nextVirtualNumber(
					patch.fixtures.map((fixture) => fixture.virtual_fixture_number),
				),
				definition,
				universe: null,
				address: null,
				layer_id: "default",
			});
			const placed = await patch.patchFixtures([candidate]);
			if (!placed?.length) throw new Error(`The show did not take the ${part.label}.`);
			setOpen(null);
			setGroup(null);
			onPlaced(placed[0].fixtureId);
		} catch (reason) {
			onError(reason);
		} finally {
			setPlacing(false);
		}
	}

	useEffect(() => {
		if (request === handled.current) return;
		handled.current = request;
		if (kind === "venue") return;
		if (kind === "curtain")
			void place({ profileId: PARAMETRIC_CURTAIN_PROFILE_ID, label: "curtain" });
		else {
			setGroup(null);
			setOpen(kind);
		}
	});

	if (!open) return null;
	const groups = open === "truss" ? TRUSS_TYPES : STAGE_TYPES;
	const close = () => {
		setOpen(null);
		setGroup(null);
	};
	const chooseGroup = (next: VenuePartGroup) => {
		if (next.parts.length === 1) void place(next.parts[0]);
		else setGroup(next);
	};
	return (
		<ModalFrame
			ariaLabel={TITLES[open].title}
			dialogClassName="cad-add-part-modal"
			title={group ? `${TITLES[open].title} · ${group.label}` : TITLES[open].title}
			closeLabel={`Close ${TITLES[open].title}`}
			onClose={close}
		>
			<div className="cad-add-part-body" aria-busy={placing || undefined}>
				<header className="cad-add-part-step">
					{group ? (
						<Button onClick={() => setGroup(null)}>Back</Button>
					) : null}
					<h3>{group ? group.partsLabel : TITLES[open].groupsLabel}</h3>
				</header>
				<div className="cad-part-grid" role="list">
					{group
						? group.parts.map((part) => (
								<div role="listitem" key={part.id}>
									<PartTile
										label={part.label}
										detail={part.detail}
										definition={definitionForProfile(definitions, part.profileId)}
										onChoose={() => void place(part)}
									/>
								</div>
							))
						: groups.map((each) => (
								<div role="listitem" key={each.id}>
									<PartTile
										label={each.label}
										detail={
											each.parts.length === 1
												? each.parts[0].label
												: `${each.parts.length} ${each.partsLabel.toLowerCase()}s`
										}
										definition={
											each.parts.length
												? definitionForProfile(definitions, each.parts[0].profileId)
												: undefined
										}
										onChoose={() => chooseGroup(each)}
									/>
								</div>
							))}
				</div>
			</div>
		</ModalFrame>
	);
}
