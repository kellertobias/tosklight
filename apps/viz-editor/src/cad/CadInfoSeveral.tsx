/**
 * Info while several elements are selected.
 *
 * **Generic** lists them by ID, name, model and patch, each a click away from being the only one
 * selected. **Placement** edits them together: each field shows the value they share, or the ends of
 * an even spread as `first THRU last`, and takes a single value for all or a range to spread over the
 * selection in the order it was made. **Placement Assistant** lays them out along a line, in a grid
 * or around a circle. Bracket angle and barn doors are a lamp's, so Venue objects keep theirs.
 */
import type { PatchFixtureProjection } from "@tosklight/patch";
import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import type { SelectedElement } from "./CadDeleteSelection";
import { CommitText } from "./cadFields";
import { CadPlacementAssistant } from "./CadPlacementAssistant";
import { describeThru, parseThru, spreadThru } from "./thruValues";
import "./cadInfo.css";

const transport = new TauriPatchTransport();

export function SelectedElementList({
	elements,
	onSelect,
}: {
	elements: readonly SelectedElement[];
	onSelect(id: string): void;
}) {
	return (
		<div className="cad-selected-list">
			<p>{elements.length} elements selected.</p>
			<div className="cad-selected-row cad-selected-head" aria-hidden="true">
				<span>ID</span>
				<span>Name</span>
				<span>Model</span>
				<span>Patch</span>
			</div>
			<ul aria-label="Selected elements">
				{elements.map((element) => (
					<li key={element.id}>
						<button
							type="button"
							className="cad-selected-row"
							title="Select only this element"
							onClick={() => onSelect(element.id)}
						>
							<b>{element.displayId}</b>
							<span>
								{element.name}
								{element.placements > 1 ? <small> · {element.placements} copies</small> : null}
							</span>
							<span>{element.model}</span>
							<span>{element.patch}</span>
						</button>
					</li>
				))}
			</ul>
		</div>
	);
}

interface ThruFieldSpec {
	id: string;
	label: string;
	ariaLabel: string;
	digits: number;
	/** Only a lamp has it; Venue objects in the selection are left out of the spread. */
	lampsOnly?: boolean;
	/** Empty is a value of its own — no barn doors fitted — rather than a refused edit. */
	optional?: boolean;
	read(fixture: PatchFixtureProjection): number | null;
	write(fixture: PatchFixtureProjection, value: number | null): PatchFixtureProjection;
}

const AXES = ["x", "y", "z"] as const;

const THRU_FIELDS: readonly ThruFieldSpec[] = [
	...AXES.map(
		(axis): ThruFieldSpec => ({
			id: `position-${axis}`,
			label: `${axis.toUpperCase()} (m)`,
			ariaLabel: `Position ${axis.toUpperCase()}`,
			digits: 3,
			read: (fixture) => fixture.location[axis] / 1000,
			write: (fixture, metres) => ({
				...fixture,
				location: { ...fixture.location, [axis]: Math.round((metres ?? 0) * 1000) },
			}),
		}),
	),
	...AXES.map(
		(axis): ThruFieldSpec => ({
			id: `rotation-${axis}`,
			label: `Rot ${axis.toUpperCase()} (°)`,
			ariaLabel: `Rotation ${axis.toUpperCase()}`,
			digits: 1,
			read: (fixture) => fixture.rotation[axis],
			write: (fixture, degrees) => ({
				...fixture,
				rotation: { ...fixture.rotation, [axis]: degrees ?? 0 },
			}),
		}),
	),
	{
		id: "bracket",
		label: "Bracket angle (°)",
		ariaLabel: "Bracket angle",
		digits: 1,
		lampsOnly: true,
		read: (fixture) => fixture.bracketAngle ?? 0,
		write: (fixture, degrees) => ({ ...fixture, bracketAngle: degrees ?? 0 }),
	},
	{
		id: "barndoors",
		label: "Barndoors (°)",
		ariaLabel: "Barndoors",
		digits: 1,
		lampsOnly: true,
		optional: true,
		read: (fixture) => fixture.shaperAngle ?? null,
		write: (fixture, degrees) => ({ ...fixture, shaperAngle: degrees }),
	},
];

/** The selected fixtures as the patch holds them, in selection order. */
function useSelectedFixtures(ids: readonly string[], sceneRevision: number) {
	const [fixtures, setFixtures] = useState<PatchFixtureProjection[]>([]);
	const key = ids.join(",");
	useEffect(() => {
		let current = true;
		const wanted = key.split(",").filter(Boolean);
		documentSession
			.patchSnapshot()
			.then((snapshot) => {
				if (!current) return;
				const byId = new Map(snapshot.fixtures.map((each) => [each.fixtureId, each]));
				setFixtures(wanted.flatMap((id) => byId.get(id) ?? []));
			})
			.catch(() => current && setFixtures([]));
		return () => {
			current = false;
		};
	}, [key, sceneRevision]);
	return [fixtures, setFixtures] as const;
}

function ThruField({
	spec,
	targets,
	onWrite,
}: {
	spec: ThruFieldSpec;
	targets: readonly PatchFixtureProjection[];
	onWrite(next: PatchFixtureProjection[]): void;
}) {
	const { text, mixed } = describeThru(targets.map(spec.read), spec.digits);
	return (
		<CommitText
			label={spec.label}
			ariaLabel={spec.ariaLabel}
			placeholder={mixed ? "Mixed" : spec.optional ? "None" : undefined}
			value={text}
			accepts={(draft) => (spec.optional && draft.trim() === "") || parseThru(draft) != null}
			onCommit={(draft) => {
				const range = parseThru(draft);
				if (!range && !spec.optional) return;
				const values = range ? spreadThru(range, targets.length) : targets.map(() => null);
				onWrite(targets.map((fixture, index) => spec.write(fixture, values[index])));
			}}
		/>
	);
}

export function SeveralPlacement({
	elements,
	sceneRevision,
	onError,
}: {
	elements: readonly SelectedElement[];
	sceneRevision: number;
	onError(reason: unknown): void;
}) {
	const [fixtures, setFixtures] = useSelectedFixtures(
		elements.map((element) => element.id),
		sceneRevision,
	);
	const [assisting, setAssisting] = useState(false);
	const lamps = new Set(elements.filter((element) => element.isFixture).map((element) => element.id));

	async function write(next: PatchFixtureProjection[]) {
		if (!next.length) return;
		const before = fixtures;
		const written = new Map(next.map((fixture) => [fixture.fixtureId, fixture]));
		setFixtures(fixtures.map((fixture) => written.get(fixture.fixtureId) ?? fixture));
		try {
			await transport.patchFixtures("", 0, {
				requestId: crypto.randomUUID(),
				fixtures: next,
				removeFixtureIds: [],
			});
		} catch (reason) {
			setFixtures(before);
			onError(reason);
		}
	}

	return (
		<div className="cad-several-placement">
			<div className="cad-thru-fields">
				{THRU_FIELDS.map((spec) => {
					const targets = spec.lampsOnly
						? fixtures.filter((fixture) => lamps.has(fixture.fixtureId))
						: fixtures;
					return targets.length ? (
						<ThruField key={spec.id} spec={spec} targets={targets} onWrite={(next) => void write(next)} />
					) : null;
				})}
			</div>
			<p className="cad-thru-hint">
				One value sets every element; <kbd>1 THRU 5</kbd> spreads from the first selected to the last.
			</p>
			<Button disabled={!fixtures.length} onClick={() => setAssisting(true)}>
				Placement Assistant
			</Button>
			{assisting ? (
				<CadPlacementAssistant
					current={fixtures.map((fixture) => fixture.location)}
					onClose={() => setAssisting(false)}
					onApply={(positions) => {
						setAssisting(false);
						void write(fixtures.map((fixture, index) => ({ ...fixture, location: positions[index] })));
					}}
				/>
			) : null}
		</div>
	);
}
