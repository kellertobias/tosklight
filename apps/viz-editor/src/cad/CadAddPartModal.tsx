/**
 * The dialog behind **Add truss**, **Add stage element** and **Add primitive**, and the direct add
 * behind **Add curtain**.
 *
 * Choosing takes two steps: the truss section or what a stage element stands on, then the part or
 * platform size, each shown by its picture on a dark ground. The chosen part is placed at the stage
 * origin like any other Venue object, and the CAD screen selects it so Info opens to place and size
 * it. A step with a single choice is skipped, so a primitive — box, cylinder or ball — is one step.
 *
 * The dialog reads the fixture library itself each time it opens and writes the placement straight to
 * the show, so it offers what this computer's library holds now and a refusal carries the show's own
 * reason.
 */
import {
	type FixtureDefinition,
	mergeFixtureDefinitions,
	newPatchFixtureCandidate,
} from "@tosklight/patch";
import { ModalFrame } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import {
	definitionForProfile,
	nextVirtualNumber,
	PARAMETRIC_CURTAIN_PROFILE_ID,
	PRIMITIVE_TYPES,
	previewOf,
	STAGE_TYPES,
	TRUSS_TYPES,
	type VenuePart,
	type VenuePartGroup,
} from "./venueParts";
import "./cadAddParts.css";

/** What the add buttons ask for; `venue` is any other object and is left to the fixture library. */
export type CadPartKind = "truss" | "stage" | "curtain" | "primitive" | "venue";

type ChosenKind = "truss" | "stage" | "primitive";

const TITLES: Record<ChosenKind, { title: string; groupsLabel: string }> = {
	truss: { title: "Add truss", groupsLabel: "Truss type" },
	stage: { title: "Add stage element", groupsLabel: "Feet" },
	primitive: { title: "Add primitive", groupsLabel: "Shape" },
};

const GROUPS: Record<ChosenKind, readonly VenuePartGroup[]> = {
	truss: TRUSS_TYPES,
	stage: STAGE_TYPES,
	primitive: PRIMITIVE_TYPES,
};

const transport = new TauriPatchTransport();

/** The library as the dialog last read it: still reading, read, or refused with a reason. */
type Library =
	| { state: "loading" }
	| { state: "ready"; definitions: readonly FixtureDefinition[] }
	| { state: "failed"; reason: string };

async function readLibrary(): Promise<readonly FixtureDefinition[]> {
	return mergeFixtureDefinitions(await documentSession.fixtureProfiles(), []);
}

/** Places one part at the stage origin with the first free virtual ID, and returns its fixture ID. */
async function placePart(definition: FixtureDefinition): Promise<string> {
	const snapshot = await documentSession.patchSnapshot();
	const candidate = newPatchFixtureCandidate({
		name: definition.name,
		fixture_number: null,
		virtual_fixture_number: nextVirtualNumber(
			snapshot.fixtures.map((fixture) => fixture.virtualFixtureNumber),
		),
		definition,
		universe: null,
		address: null,
		layer_id: "default",
	});
	await transport.patchFixtures(snapshot.showId, snapshot.patchRevision, {
		requestId: crypto.randomUUID(),
		fixtures: [candidate.input],
		removeFixtureIds: [],
	});
	return candidate.fixture.fixture_id;
}

function PartTile({
	label,
	detail,
	definition,
	library,
	onChoose,
}: {
	label: string;
	detail?: string;
	definition: FixtureDefinition | undefined;
	library: Library;
	onChoose(): void;
}) {
	const preview = previewOf(definition);
	const missing =
		library.state === "loading"
			? "Loading the fixture library…"
			: library.state === "failed"
				? "The fixture library could not be read"
				: "Not in this library";
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
			<small>{definition ? (detail ?? definition.name) : missing}</small>
		</button>
	);
}

/** The step's choices: the groups first, then the parts of the chosen group. */
function PartGrid({
	group,
	groups,
	library,
	onPlace,
	onChooseGroup,
}: {
	group: VenuePartGroup | null;
	groups: readonly VenuePartGroup[];
	library: Library;
	onPlace(part: VenuePart): void;
	onChooseGroup(group: VenuePartGroup): void;
}) {
	const find = (profileId: string) =>
		library.state === "ready" ? definitionForProfile(library.definitions, profileId) : undefined;
	return (
		<div className="cad-part-grid" role="list">
			{group
				? group.parts.map((part) => (
						<div role="listitem" key={part.id}>
							<PartTile
								label={part.label}
								detail={part.detail}
								library={library}
								definition={find(part.profileId)}
								onChoose={() => onPlace(part)}
							/>
						</div>
					))
				: groups.map((each) => (
						<div role="listitem" key={each.id}>
							<PartTile
								label={each.label}
								detail={
									each.parts.length === 1
										? (each.parts[0].detail ?? each.parts[0].label)
										: `${each.parts.length} ${each.partsLabel.toLowerCase()}s`
								}
								library={library}
								definition={each.parts.length ? find(each.parts[0].profileId) : undefined}
								onChoose={() => onChooseGroup(each)}
							/>
						</div>
					))}
		</div>
	);
}

export function CadAddPartModal({
	kind,
	request,
	onPlaced,
	onError,
}: {
	kind: CadPartKind;
	/** Bumped on every press of an add button; each press opens or places once. */
	request: number;
	onPlaced(fixtureId: string): void;
	onError(reason: unknown): void;
}) {
	const [open, setOpen] = useState<ChosenKind | null>(null);
	const [group, setGroup] = useState<VenuePartGroup | null>(null);
	const [library, setLibrary] = useState<Library>({ state: "loading" });
	const [placing, setPlacing] = useState(false);
	const handled = useRef(request);

	async function place(part: Pick<VenuePart, "profileId" | "label">, known?: Library) {
		const current = known ?? library;
		const definition =
			current.state === "ready" ? definitionForProfile(current.definitions, part.profileId) : undefined;
		if (!definition) {
			onError(`The ${part.label} is not in this computer's fixture library.`);
			return;
		}
		setPlacing(true);
		try {
			const fixtureId = await placePart(definition);
			setOpen(null);
			setGroup(null);
			onPlaced(fixtureId);
		} catch (reason) {
			onError(`The show refused the ${part.label}: ${String(reason)}`);
		} finally {
			setPlacing(false);
		}
	}

	/** Reads the library afresh, and hands what it read to the caller as well as the dialog. */
	async function refreshLibrary(): Promise<Library> {
		setLibrary({ state: "loading" });
		const next: Library = await readLibrary().then(
			(definitions) => ({ state: "ready", definitions }),
			(reason) => ({ state: "failed", reason: String(reason) }),
		);
		setLibrary(next);
		if (next.state === "failed") onError(`The fixture library could not be read: ${next.reason}`);
		return next;
	}

	useEffect(() => {
		if (request === handled.current) return;
		handled.current = request;
		if (kind === "venue") return;
		if (kind === "curtain") {
			void refreshLibrary().then((read) =>
				place({ profileId: PARAMETRIC_CURTAIN_PROFILE_ID, label: "curtain" }, read),
			);
			return;
		}
		setGroup(null);
		setOpen(kind);
		void refreshLibrary();
	});

	if (!open) return null;
	const groups = GROUPS[open];
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
			// The second step returns to the first from the title, beside the close button.
			groups={
				group
					? [
							{
								id: "cad-add-part-back",
								actions: [
									{
										id: "back",
										label: "Back",
										icon: (
											<svg className="cad-add-part-back-icon" viewBox="0 0 16 16" aria-hidden="true">
												<path d="M10 3 5 8l5 5" />
											</svg>
										),
										onPress: () => setGroup(null),
									},
								],
							},
						]
					: undefined
			}
			onClose={close}
		>
			<div className="cad-add-part-body" aria-busy={placing || library.state === "loading" || undefined}>
				<header className="cad-add-part-step">
					<h3>{group ? group.partsLabel : TITLES[open].groupsLabel}</h3>
				</header>
				<PartGrid
					group={group}
					groups={groups}
					library={library}
					onPlace={(part) => void place(part)}
					onChooseGroup={chooseGroup}
				/>
			</div>
		</ModalFrame>
	);
}
