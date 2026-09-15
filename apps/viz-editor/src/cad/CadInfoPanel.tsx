/**
 * The Info panel: the element selected in the drawing, edited directly.
 *
 * Name, notes, where it stands and how it is turned are the element's own; scale only for the Venue
 * objects that can be drawn larger or smaller. Each change is written as soon as its field is left,
 * and the drawing redraws from the show like any other change, so the panel never holds a second copy
 * of the element.
 */
import type { PatchFixtureProjection } from "@tosklight/patch";
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

const transport = new TauriPatchTransport();

export function CadInfoPanel({
	entity,
	selectionCount,
	sceneRevision,
	onError,
}: {
	/** The one selected element, or null when several or none are. */
	entity: CadEntity | null;
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

	async function write(change: Partial<PatchFixtureProjection>) {
		if (!fixture) return;
		const next = { ...fixture, ...change };
		setFixture(next);
		try {
			await transport.patchFixtures("", 0, {
				requestId: crypto.randomUUID(),
				fixtures: [next],
				removeFixtureIds: [],
			});
		} catch (reason) {
			setFixture(fixture);
			onError(reason);
		}
	}

	const position = fixture?.location ?? {
		x: entity.positionMillimetres[0],
		y: entity.positionMillimetres[1],
		z: entity.positionMillimetres[2],
	};
	const rotation = fixture?.rotation ?? {
		x: entity.rotationDegrees[0],
		y: entity.rotationDegrees[1],
		z: entity.rotationDegrees[2],
	};
	const editable = Boolean(fixture);

	return (
		<section className="cad-info" aria-label="Info">
			<h3>Info</h3>
			<fieldset disabled={!editable}>
				<CommitText
					label="Name"
					value={fixture?.name ?? entity.name}
					onCommit={(name) => name.trim() && void write({ name: name.trim() })}
				/>
				<CommitTextArea
					label="Notes"
					value={note}
					onCommit={(next) => {
						setNote(next);
						documentSession
							.saveFixtureNote({ fixtureId: entity.logicalFixtureId, note: next })
							.catch(onError);
					}}
				/>
				<div className="cad-info-vector" role="group" aria-label="Position">
					<span>Position (m)</span>
					{AXES.map((axis) => (
						<CommitNumber
							key={axis}
							label={axis.toUpperCase()}
							ariaLabel={`Position ${axis.toUpperCase()}`}
							value={position[axis] / 1000}
							onCommit={(metres) =>
								void write({
									location: { ...position, [axis]: Math.round(metres * 1000) },
								})
							}
						/>
					))}
				</div>
				<div className="cad-info-vector" role="group" aria-label="Rotation">
					<span>Rotation (°)</span>
					{AXES.map((axis) => (
						<CommitNumber
							key={axis}
							label={axis.toUpperCase()}
							ariaLabel={`Rotation ${axis.toUpperCase()}`}
							digits={1}
							value={rotation[axis]}
							onCommit={(degrees) => void write({ rotation: { ...rotation, [axis]: degrees } })}
						/>
					))}
				</div>
				{supportsScale(entity) ? (
					<CommitNumber
						label="Scale"
						value={fixture?.modelScale ?? 1}
						min={0.01}
						max={100}
						onCommit={(scale) => void write({ modelScale: scale === 1 ? null : scale })}
					/>
				) : null}
			</fieldset>
		</section>
	);
}
