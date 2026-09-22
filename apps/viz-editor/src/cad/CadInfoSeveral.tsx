/**
 * Info while several elements are selected.
 *
 * **Generic** lists them by ID, name, model and patch, each a click away from being the only one
 * selected. **Placement** edits them together: each field shows the value they share, or the ends of
 * an even spread as `first THRU last`, and takes a single value for all or a range to spread over the
 * selection in the order it was made. **Placement Assistant** lays them out along a line, in a grid
 * or around a circle. Bracket angle and barn doors are a lamp's, so Venue objects keep theirs.
 *
 * When every selected element was patched from the same model, **Shared model** follows: that
 * model's own controls — a stage element's height, a truss's length, a placed model's scale, the
 * options it is built with — named so it is plain what they reach, and written to the whole
 * selection at once. A selection of mixed models has no such section.
 */
import type { PatchFixtureProjection, PatchProfileRevision } from "@tosklight/patch";
import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";
import type { SelectedElement } from "./CadDeleteSelection";
import { CommitText } from "./cadFields";
import { CadPlacementAssistant } from "./CadPlacementAssistant";
import { SceneryParameters } from "./CadInfoFields";
import {
	type SharedModel,
	sharedModel,
	sharedModelFields,
	type ThruFieldSpec,
	THRU_FIELDS,
} from "./thruFields";
import { describeRange, describeThru, parseThru, spreadThru } from "./thruValues";
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

/**
 * The selected fixtures as the patch holds them, in selection order, with the profiles they were
 * patched from — the same snapshot answers both, so the panel asks once.
 */
function useSelectedFixtures(ids: readonly string[], sceneRevision: number) {
	const [fixtures, setFixtures] = useState<PatchFixtureProjection[]>([]);
	const [revisions, setRevisions] = useState<readonly PatchProfileRevision[]>([]);
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
				setRevisions(snapshot.profileRevisions ?? []);
			})
			.catch(() => {
				if (!current) return;
				setFixtures([]);
				setRevisions([]);
			});
		return () => {
			current = false;
		};
	}, [key, sceneRevision]);
	return [fixtures, setFixtures, revisions] as const;
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
	const { text, range } = describeThru(targets.map(spec.read), spec.digits);
	return (
		<CommitText
			label={spec.label}
			ariaLabel={spec.ariaLabel}
			placeholder={
				range ? describeRange(range, spec.digits, spec.unit) : spec.optional ? "None" : undefined
			}
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

/**
 * What the one model behind the whole selection lets the operator set, named so it is plain what
 * the controls reach. A selection of mixed models has no such section and keeps the fields above.
 */
function SharedModelFields({
	model,
	count,
	fallbackLabel,
	fields,
	fixtures,
	onWrite,
}: {
	model: SharedModel;
	count: number;
	fallbackLabel: string;
	fields: readonly ThruFieldSpec[];
	fixtures: readonly PatchFixtureProjection[];
	onWrite(next: PatchFixtureProjection[]): void;
}) {
	const parameters = model.scenery;
	if (!fields.length && !parameters) return null;
	// Every selected element carries the same options, so the first one's stand for the selection.
	const options = fixtures[0]?.sceneryOptions ?? null;
	return (
		<section className="cad-shared-model" aria-label="Shared model">
			<p className="cad-shared-model-name">
				All {count} × {model.label || fallbackLabel}
			</p>
			<div className="cad-thru-fields">
				{fields.map((spec) => (
					<ThruField key={spec.id} spec={spec} targets={fixtures} onWrite={onWrite} />
				))}
			</div>
			{parameters ? (
				<SceneryParameters
					scenery={parameters}
					options={options}
					shared=" (all selected)"
					onCommit={(sceneryOptions) =>
						onWrite(fixtures.map((fixture) => ({ ...fixture, sceneryOptions })))
					}
				/>
			) : null}
		</section>
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
	const [fixtures, setFixtures, revisions] = useSelectedFixtures(
		elements.map((element) => element.id),
		sceneRevision,
	);
	const [assisting, setAssisting] = useState(false);
	const lamps = new Set(elements.filter((element) => element.isFixture).map((element) => element.id));
	const model = sharedModel(fixtures, revisions);
	const allVenue = elements.length > 0 && elements.every((element) => !element.isFixture);
	const modelFields = model ? sharedModelFields(model, allVenue) : [];

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
			{model ? (
				<SharedModelFields
					model={model}
					count={fixtures.length}
					fallbackLabel={elements[0]?.model ?? "the same model"}
					fields={modelFields}
					fixtures={fixtures}
					onWrite={(next) => void write(next)}
				/>
			) : null}
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
