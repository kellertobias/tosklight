/**
 * The Info panel: the element selected in the drawing, edited directly.
 *
 * Name, notes, where it stands and how it is turned are the element's own. A generated Venue object —
 * a truss, a curtain, a stage element — is sized by the measurements its profile lets the operator
 * set, in metres within the profile's range; a placed 3D model is drawn larger or smaller by its scale.
 * Each change is written as soon as its field is left, and the drawing redraws from the show like any
 * other change, so the panel never holds a second copy of the element.
 *
 * A multi-patched fixture stands in several places. Each copy has its own name, position and
 * rotation, so Info edits the copy that was clicked — or the one chosen under **Copy** — and leaves
 * the others where they are. Notes, size and scale belong to the fixture and so to every copy.
 */
import type {
	FixtureProfileScenery,
	PatchFixtureProjection,
	PatchMultiPatch,
} from "@tosklight/patch";
import { useEffect, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import { CommitNumber, CommitText, CommitTextArea } from "./cadFields";
import type { CadEntity } from "./types";

type Axis = "x" | "y" | "z";
const AXES: readonly Axis[] = ["x", "y", "z"];

/** The measurements of a generated object, with the key the patch stores each under. */
const SIZE_AXES = [
	{ axis: "width", key: "x", label: "Width" },
	{ axis: "height", key: "y", label: "Height" },
	{ axis: "depth", key: "z", label: "Depth" },
] as const;

/** Whether an element can be drawn at another size: placed Venue objects, except crowd areas. */
export function supportsScale(entity: CadEntity): boolean {
	return entity.kind === "venue" && entity.scenery?.kind !== "crowd";
}

/** Whether a generated object has any measurement the operator sets. */
export function hasAdjustableSize(scenery: FixtureProfileScenery | null | undefined) {
	return Boolean(scenery && SIZE_AXES.some(({ axis }) => scenery.adjustable[axis]));
}

type Placement = Pick<PatchMultiPatch, "name" | "location" | "rotation">;

/** The fixture with one placement changed: its own when `copyId` is null, else that copy's. */
export function withPlacement(
	fixture: PatchFixtureProjection,
	copyId: string | null,
	change: Partial<Placement>,
): PatchFixtureProjection {
	if (!copyId) return { ...fixture, ...change };
	return {
		...fixture,
		multipatch: fixture.multipatch.map((copy) =>
			copy.id === copyId ? { ...copy, ...change } : copy,
		),
	};
}

/** The size an object is placed at in metres: what the patch stores, else its profile's default. */
export function placedSize(
	fixture: PatchFixtureProjection,
	scenery: FixtureProfileScenery,
): Record<Axis, number> {
	const stored = fixture.scenerySizeMetres;
	const axis = (key: Axis) =>
		stored && Number.isFinite(stored[key]) && stored[key] > 0
			? stored[key] / 1000
			: scenery.default_size_metres[key];
	return { x: axis("x"), y: axis("y"), z: axis("z") };
}

const transport = new TauriPatchTransport();

/** The selected fixture as the patch holds it, with its note and what its profile generates. */
function useInfoFixture(fixtureId: string | null, sceneRevision: number) {
	const [fixture, setFixture] = useState<PatchFixtureProjection | null>(null);
	const [scenery, setScenery] = useState<FixtureProfileScenery | null>(null);
	const [note, setNote] = useState("");
	useEffect(() => {
		let current = true;
		if (!fixtureId) {
			setFixture(null);
			setScenery(null);
			return;
		}
		Promise.resolve()
			.then(() =>
				Promise.all([documentSession.patchSnapshot(), documentSession.fixtureNotes()]),
			)
			.then(([snapshot, notes]) => {
				if (!current) return;
				const found = snapshot.fixtures.find((each) => each.fixtureId === fixtureId) ?? null;
				setFixture(found);
				const revision = snapshot.profileRevisions?.find(
					(each) =>
						each.profileId === found?.profileId &&
						each.profileRevision === found?.profileRevision,
				);
				setScenery(revision?.profileSnapshot?.scenery ?? null);
				setNote(notes.find((each) => each.fixtureId === fixtureId)?.note ?? "");
			})
			.catch(() => current && setFixture(null));
		return () => {
			current = false;
		};
	}, [fixtureId, sceneRevision]);
	return { fixture, setFixture, scenery, note, setNote };
}

function PlacementChooser({
	entity,
	placements,
	onChoose,
}: {
	entity: CadEntity;
	placements: readonly CadEntity[];
	onChoose(entityId: string): void;
}) {
	if (placements.length < 2) return null;
	return (
		<label className="cad-field">
			<span>Copy</span>
			<select
				className="ui-input"
				aria-label="Copy"
				value={entity.id}
				onChange={(event) => onChoose(event.currentTarget.value)}
			>
				{placements.map((placement, index) => (
					<option key={placement.id} value={placement.id}>
						{index === 0 ? "Original" : `Copy ${index}`} · {placement.dmxAddress}
					</option>
				))}
			</select>
		</label>
	);
}

/** X, Y and Z of a position or rotation, each committed on its own. */
function VectorFields({
	label,
	unit,
	digits,
	value,
	show,
	onCommit,
}: {
	label: string;
	unit: string;
	digits?: number;
	value: Record<Axis, number>;
	show(stored: number): number;
	onCommit(axis: Axis, shown: number): void;
}) {
	return (
		<div className="cad-info-vector" role="group" aria-label={label}>
			<span>{label}</span>
			{AXES.map((axis) => (
				<CommitNumber
					key={axis}
					label={axis.toUpperCase()}
					ariaLabel={`${label} ${axis.toUpperCase()}`}
					digits={digits}
					unit={unit}
					value={show(value[axis])}
					onCommit={(next) => onCommit(axis, next)}
				/>
			))}
		</div>
	);
}

/** The measurements a generated object's profile lets the operator set, in metres within its range. */
function SizeFields({
	fixture,
	scenery,
	shared,
	onWrite,
}: {
	fixture: PatchFixtureProjection;
	scenery: FixtureProfileScenery;
	shared: string;
	onWrite(next: PatchFixtureProjection): void;
}) {
	const size = placedSize(fixture, scenery);
	const axes = SIZE_AXES.filter(({ axis }) => scenery.adjustable[axis]);
	return (
		<div className="cad-info-vector" role="group" aria-label="Size">
			<span>Size{shared}</span>
			{axes.map(({ key, label }) => (
				<CommitNumber
					key={key}
					label={label}
					ariaLabel={label}
					unit="m"
					min={scenery.minimum_size_metres[key]}
					max={scenery.maximum_size_metres[key]}
					value={size[key]}
					onCommit={(metres) => {
						const next = { ...size, [key]: metres };
						onWrite({
							...fixture,
							scenerySizeMetres: {
								x: Math.round(next.x * 1000),
								y: Math.round(next.y * 1000),
								z: Math.round(next.z * 1000),
							},
						});
					}}
				/>
			))}
		</div>
	);
}

export function CadInfoPanel({
	entity,
	placements = [],
	onChoosePlacement,
	selectionCount,
	sceneRevision,
	onError,
}: {
	/** The one selected placement, or null when several fixtures or none are selected. */
	entity: CadEntity | null;
	/** Every placement of the selected fixture: the fixture first, then its multi-patch copies. */
	placements?: readonly CadEntity[];
	onChoosePlacement?(entityId: string): void;
	selectionCount: number;
	/** Bumped whenever the drawing changes, so the panel reads the element again. */
	sceneRevision: number;
	onError(reason: unknown): void;
}) {
	const { fixture, setFixture, scenery, note, setNote } = useInfoFixture(
		entity?.logicalFixtureId ?? null,
		sceneRevision,
	);

	if (!entity)
		return (
			<section className="cad-info" aria-label="Info">
				<h3>Info</h3>
				<p>{selectionCount} elements selected.</p>
			</section>
		);

	async function write(next: PatchFixtureProjection) {
		if (!fixture) return;
		const before = fixture;
		setFixture(next);
		try {
			await transport.patchFixtures("", 0, {
				requestId: crypto.randomUUID(),
				fixtures: [next],
				removeFixtureIds: [],
			});
		} catch (reason) {
			setFixture(before);
			onError(reason);
		}
	}

	const copyId = entity.id === entity.logicalFixtureId ? null : entity.id;
	const copy = copyId ? fixture?.multipatch?.find((each) => each.id === copyId) : undefined;
	const placement: Placement | null = fixture ? (copyId ? (copy ?? null) : fixture) : null;
	const place = (change: Partial<Placement>) =>
		fixture && placement && void write(withPlacement(fixture, copyId, change));
	const position = placement?.location ?? {
		x: entity.positionMillimetres[0],
		y: entity.positionMillimetres[1],
		z: entity.positionMillimetres[2],
	};
	const rotation = placement?.rotation ?? {
		x: entity.rotationDegrees[0],
		y: entity.rotationDegrees[1],
		z: entity.rotationDegrees[2],
	};
	const shared = placements.length > 1 ? " (all copies)" : "";
	// A copy with no name of its own is shown under the fixture's name.
	const fixtureName = fixture?.name ?? entity.name;
	const name = copyId ? copy?.name.trim() || fixtureName : fixtureName;

	return (
		<section className="cad-info" aria-label="Info">
			<h3>Info</h3>
			<PlacementChooser entity={entity} placements={placements} onChoose={(id) => onChoosePlacement?.(id)} />
			<fieldset disabled={!placement}>
				<CommitText label="Name" value={name} onCommit={(next) => next.trim() && place({ name: next.trim() })} />
				<CommitTextArea
					label={`Notes${shared}`}
					value={note}
					onCommit={(next) => {
						setNote(next);
						documentSession
							.saveFixtureNote({ fixtureId: entity.logicalFixtureId, note: next })
							.catch(onError);
					}}
				/>
				<VectorFields
					label="Position"
					unit="m"
					value={position}
					show={(millimetres) => millimetres / 1000}
					onCommit={(axis, metres) =>
						place({ location: { ...position, [axis]: Math.round(metres * 1000) } })
					}
				/>
				<VectorFields
					label="Rotation"
					unit="°"
					digits={1}
					value={rotation}
					show={(degrees) => degrees}
					onCommit={(axis, degrees) => place({ rotation: { ...rotation, [axis]: degrees } })}
				/>
				{fixture && scenery && hasAdjustableSize(scenery) ? (
					<SizeFields fixture={fixture} scenery={scenery} shared={shared} onWrite={(next) => void write(next)} />
				) : supportsScale(entity) ? (
					<CommitNumber
						label={`Scale${shared}`}
						ariaLabel="Scale"
						unit="×"
						value={fixture?.modelScale ?? 1}
						min={0.01}
						max={100}
						onCommit={(scale) =>
							fixture && void write({ ...fixture, modelScale: scale === 1 ? null : scale })
						}
					/>
				) : null}
			</fieldset>
		</section>
	);
}
