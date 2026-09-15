/**
 * The Info panel: the element selected in the drawing, edited directly.
 *
 * Name, notes, where it stands and how it is turned are the element's own; scale only for the Venue
 * objects that can be drawn larger or smaller. Each change is written as soon as its field is left,
 * and the drawing redraws from the show like any other change, so the panel never holds a second copy
 * of the element.
 *
 * A multi-patched fixture stands in several places. Each copy has its own name, position and
 * rotation, so Info edits the copy that was clicked — or the one chosen under **Copy** — and leaves
 * the others where they are. Notes and scale belong to the fixture and so to every copy.
 */
import type { PatchFixtureProjection, PatchMultiPatch } from "@tosklight/patch";
import { useEffect, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import { CommitNumber, CommitText, CommitTextArea } from "./cadFields";
import type { CadEntity } from "./types";

type Axis = "x" | "y" | "z";
const AXES: readonly Axis[] = ["x", "y", "z"];

/** Whether an element can be drawn at another size: placed Venue objects, except crowd areas. */
export function supportsScale(entity: CadEntity): boolean {
	return entity.kind === "venue" && entity.scenery?.kind !== "crowd";
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

const transport = new TauriPatchTransport();

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
	const [fixture, setFixture] = useState<PatchFixtureProjection | null>(null);
	const [note, setNote] = useState("");
	const fixtureId = entity?.logicalFixtureId ?? null;

	useEffect(() => {
		let current = true;
		if (!fixtureId) {
			setFixture(null);
			return;
		}
		Promise.resolve()
			.then(() =>
				Promise.all([documentSession.patchSnapshot(), documentSession.fixtureNotes()]),
			)
			.then(([snapshot, notes]) => {
				if (!current) return;
				setFixture(snapshot.fixtures.find((each) => each.fixtureId === fixtureId) ?? null);
				setNote(notes.find((each) => each.fixtureId === fixtureId)?.note ?? "");
			})
			.catch(() => current && setFixture(null));
		return () => {
			current = false;
		};
	}, [fixtureId, sceneRevision]);

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
	const name = copyId ? copy?.name.trim() || (fixture?.name ?? entity.name) : (fixture?.name ?? entity.name);

	return (
		<section className="cad-info" aria-label="Info">
			<h3>Info</h3>
			<PlacementChooser
				entity={entity}
				placements={placements}
				onChoose={(id) => onChoosePlacement?.(id)}
			/>
			<fieldset disabled={!placement}>
				<CommitText
					label="Name"
					value={name}
					onCommit={(next) => next.trim() && place({ name: next.trim() })}
				/>
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
				{supportsScale(entity) ? (
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
