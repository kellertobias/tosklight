/** Authoritative venue details edited in Info > Placement, one CAD history gesture per edit. */
import type { PatchFixtureProjection, WallOpening } from "@tosklight/patch";
import { useState } from "react";
import { architectureFromFixture } from "./architectureDraft";
import { CadArchitectureFields } from "./CadArchitectureFields";
import { CadWallOpeningFields } from "./CadWallOpeningFields";

export function CadVenuePlacementSettings({
	fixture,
	selectedOpeningId,
	onSelectOpening,
	onWrite,
}: {
	fixture: PatchFixtureProjection;
	selectedOpeningId?: string | null;
	onSelectOpening?(id: string | null): void;
	onWrite(next: PatchFixtureProjection): Promise<void | boolean>;
}) {
	const [localOpening, setLocalOpening] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const architecture = fixture.sceneryOptions?.architecture;
	if (!architecture) return null;
	const draft = architectureFromFixture(fixture);
	const selected = selectedOpeningId === undefined ? localOpening : selectedOpeningId;
	const index = architecture.kind === "wall"
		? architecture.openings.findIndex((opening) => opening.id === selected)
		: -1;
	const opening = architecture.kind === "wall" ? architecture.openings[index] : undefined;
	const choose = (id: string | null) => {
		setLocalOpening(id);
		onSelectOpening?.(id);
	};
	const commit = (next: PatchFixtureProjection, onSuccess?: () => void) => {
		setBusy(true);
		void onWrite(next).then((accepted) => {
			if (accepted !== false) onSuccess?.();
		}).finally(() => setBusy(false));
	};
	const changeOpening = (next: WallOpening) => {
		if (architecture.kind !== "wall") return;
		commit({
			...fixture,
			sceneryOptions: {
				...fixture.sceneryOptions,
				architecture: {
					...architecture,
					openings: architecture.openings.map((each) => each.id === next.id ? next : each),
				},
			},
		});
	};
	const removeOpening = () => {
		if (architecture.kind !== "wall" || !opening) return;
		commit({
			...fixture,
			sceneryOptions: {
				...fixture.sceneryOptions,
				architecture: {
					...architecture,
					openings: architecture.openings.filter((each) => each.id !== opening.id),
				},
			},
		}, () => choose(null));
	};
	return (
		<fieldset disabled={busy} aria-label="Venue construction" aria-busy={busy}>
			{opening ? (
				<>
					<h4>{opening.opening.kind === "door" ? "Door" : "Window"} settings</h4>
					<CadWallOpeningFields opening={opening} index={index} onChange={changeOpening} />
					<button type="button" className="ui-button" onClick={removeOpening}>Remove opening</button>
					<button type="button" className="ui-button" onClick={() => choose(null)}>Wall settings</button>
				</>
			) : (
				<>
					<CadArchitectureFields
						draft={draft}
						showName={false}
						showPlacement={false}
						showOpenings={false}
						onChange={(next) => commit({
							...fixture,
							scenerySizeMetres: next.size === draft.size ? fixture.scenerySizeMetres : next.size,
							sceneryOptions: {
								...fixture.sceneryOptions,
								colourSrgb: next.colourSrgb,
								architecture: next.architecture,
							},
						})}
					/>
					{architecture.kind === "wall" && architecture.openings.length ? (
						<section aria-label="Windows and doors">
							<h4>Windows and doors</h4>
							{architecture.openings.map((each, i) => (
								<button key={each.id} type="button" className="ui-button" onClick={() => choose(each.id)}>
									{each.opening.kind === "door" ? "Door" : "Window"} {i + 1}
								</button>
							))}
						</section>
					) : null}
				</>
			)}
		</fieldset>
	);
}
