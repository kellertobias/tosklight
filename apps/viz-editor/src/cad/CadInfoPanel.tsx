/**
 * The Info panel: the element selected in the drawing, edited directly.
 *
 * Name, notes, where it stands and how it is turned are the element's own. A generated Venue object —
 * a truss, a curtain, a stage element — is sized by the measurements its profile lets the operator
 * set, in metres within the profile's range; a placed 3D model is drawn larger or smaller by its scale.
 * Each change is written as soon as its field is left, and the drawing redraws from the show like any
 * other change, so the panel never holds a second copy of the element.
 *
 * Two tabs divide it. **Generic** holds the name, notes and, for a lamp, where it is patched.
 * **Placement** holds position and rotation, a generated object's size and parameters or a model's
 * scale, and a lamp's bracket angle and barn doors.
 *
 * A multi-patched fixture stands in several places. Each copy has its own name, patch, position,
 * rotation, bracket and barn doors, so Info edits the copy that was clicked — or the one chosen under
 * **Copy** — and leaves the others where they are. Notes, size, parameters and scale belong to the
 * fixture and so to every copy.
 */
import type {
	FixtureProfileScenery,
	PatchFixtureProjection,
	PatchMultiPatch,
} from "@tosklight/patch";
import { type ReactNode, useEffect, useState } from "react";
import { documentSession, type ProfileUpdate } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import { CommitNumber, CommitText, CommitTextArea } from "./cadFields";
import { MountingFields, PatchFields, SceneryParameters } from "./CadInfoFields";
import { hasAdjustableSize, placedSize, SIZE_AXES } from "./sceneryAxes";
import type { CadEntity } from "./types";

export type InfoTab = "generic" | "placement";

type Axis = "x" | "y" | "z";
const AXES: readonly Axis[] = ["x", "y", "z"];


/** Whether an element can be drawn at another size: placed Venue objects, except crowd areas. */
export function supportsScale(entity: CadEntity): boolean {
	return entity.kind === "venue" && entity.scenery?.kind !== "crowd";
}

type Placement = Pick<
	PatchMultiPatch,
	"name" | "location" | "rotation" | "splitPatches" | "bracketAngle" | "shaperAngle"
>;

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
	action,
	several,
	tab = "generic",
}: {
	/** Which of Info's tabs is open. */
	tab?: InfoTab;
	/** The one selected placement, or null when several fixtures or none are selected. */
	entity: CadEntity | null;
	/** Every placement of the selected fixture: the fixture first, then its multi-patch copies. */
	placements?: readonly CadEntity[];
	onChoosePlacement?(entityId: string): void;
	selectionCount: number;
	/** Bumped whenever the drawing changes, so the panel reads the element again. */
	sceneRevision: number;
	onError(reason: unknown): void;
	/** A button beside the heading, such as delete. */
	action?: ReactNode;
	/** What Info shows while several elements are selected; a count when absent. */
	several?: ReactNode;
}) {
	const { fixture, setFixture, scenery, note, setNote } = useInfoFixture(
		entity?.logicalFixtureId ?? null,
		sceneRevision,
	);
	const heading = (
		<header className="cad-info-header">
			<h3>Info</h3>
			{action}
		</header>
	);

	if (!entity)
		return (
			<section className="cad-info" aria-label="Info">
				{heading}
				{several ?? <p>{selectionCount} elements selected.</p>}
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
	const place = (change: Partial<Placement>) => {
		if (fixture && placement) void write(withPlacement(fixture, copyId, change));
	};
	const shared = placements.length > 1 ? " (all copies)" : "";
	// A copy with no name of its own is shown under the fixture's name.
	const fixtureName = fixture?.name ?? entity.name;
	const name = copyId ? copy?.name.trim() || fixtureName : fixtureName;

	return (
		<section className="cad-info" aria-label="Info">
			{heading}
			<PlacementChooser entity={entity} placements={placements} onChoose={(id) => onChoosePlacement?.(id)} />
			{tab === "generic" ? (
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
					{entity.kind !== "venue" ? (
						<PatchFields
							splits={placement?.splitPatches ?? []}
							onCommit={(splitPatches) => place({ splitPatches })}
						/>
					) : null}
				</fieldset>
			) : (
				<PlacementFields
					entity={entity}
					fixture={fixture}
					sceneRevision={sceneRevision}
					onError={onError}
					scenery={scenery}
					placement={placement}
					shared={shared}
					place={place}
					write={(next) => void write(next)}
				/>
			)}
		</section>
	);
}


/**
 * The offer to bring an element up to this computer's copy of its profile.
 *
 * It appears only when the library holds a different copy from the one the element was built with,
 * which is what happens after a shipped part is corrected. The show answers that by content rather
 * than by version number, because two libraries number their own versions independently. Nothing
 * happens on its own: an old show keeps drawing what it always drew until the operator asks.
 */
function ProfileUpdateOffer({
	fixtureId,
	sceneRevision,
	shared,
	onError,
}: {
	fixtureId: string;
	sceneRevision: number;
	shared: string;
	onError(reason: unknown): void;
}) {
	const [update, setUpdate] = useState<ProfileUpdate | null>(null);
	const [busy, setBusy] = useState(false);
	useEffect(() => {
		let current = true;
		documentSession
			.fixtureProfileUpdate(fixtureId)
			.then((found) => current && setUpdate(found))
			.catch(() => current && setUpdate(null));
		return () => {
			current = false;
		};
	}, [fixtureId, sceneRevision]);
	if (!update) return null;
	return (
		<div className="cad-info-upgrade">
			<p>
				Built from version {update.fromRevision} of its profile; this computer's library holds a
				newer one.
			</p>
			<button
				type="button"
				className="ui-button"
				disabled={busy}
				onClick={() => {
					setBusy(true);
					documentSession
						.updateFixtureProfile(fixtureId)
						.then(() => setUpdate(null))
						.catch(onError)
						.finally(() => setBusy(false));
				}}
			>
				Update to the newest version{shared}
			</button>
		</div>
	);
}

/** The Placement tab for one element: where it stands, how it is built or scaled, how it is hung. */
function PlacementFields({
	entity,
	fixture,
	sceneRevision,
	onError,
	scenery,
	placement,
	shared,
	place,
	write,
}: {
	entity: CadEntity;
	fixture: PatchFixtureProjection | null;
	sceneRevision: number;
	onError(reason: unknown): void;
	scenery: FixtureProfileScenery | null;
	placement: Placement | null;
	shared: string;
	place(change: Partial<Placement>): void;
	write(next: PatchFixtureProjection): void;
}) {
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
	return (
		<fieldset disabled={!placement}>
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
				<SizeFields fixture={fixture} scenery={scenery} shared={shared} onWrite={write} />
			) : supportsScale(entity) ? (
				<CommitNumber
					label={`Scale${shared}`}
					ariaLabel="Scale"
					unit="×"
					value={fixture?.modelScale ?? 1}
					min={0.01}
					max={100}
					onCommit={(scale) =>
						fixture && write({ ...fixture, modelScale: scale === 1 ? null : scale })
					}
				/>
			) : null}
			{fixture ? (
				<ProfileUpdateOffer
					fixtureId={fixture.fixtureId}
					sceneRevision={sceneRevision}
					shared={shared}
					onError={onError}
				/>
			) : null}
			{fixture && scenery ? (
				<SceneryParameters
					scenery={scenery}
					options={fixture.sceneryOptions}
					shared={shared}
					onCommit={(sceneryOptions) => write({ ...fixture, sceneryOptions })}
				/>
			) : null}
			{entity.kind !== "venue" ? (
				<MountingFields
					bracketAngle={placement?.bracketAngle ?? entity.bracketAngle ?? 0}
					shaperAngle={placement?.shaperAngle ?? null}
					onCommit={place}
				/>
			) : null}
		</fieldset>
	);
}
