import type { PatchFixtureProjection } from "@tosklight/patch";
import { ErrorAlert, formatErrorDetails, ModalFrame } from "@tosklight/ui";
import { useState } from "react";
import { documentSession } from "../document/session";
import {
	ARCHITECTURE_PROFILE,
	type ArchitectureKind,
	architectureFromFixture,
	newArchitecture,
	snapWallDraft,
} from "./architectureDraft";
import { CadArchitectureFields } from "./CadArchitectureFields";
import { placeProfile } from "./cadPlacement";
import { cadSession } from "./session";
import "./cadArchitecture.css";

export function CadArchitectureModal({
	fixture,
	onClose,
	onPlaced,
	onSaved,
}: {
	fixture?: PatchFixtureProjection;
	onClose(): void;
	onPlaced?(ids: readonly string[]): void;
	onSaved?(next: PatchFixtureProjection): void;
}) {
	const [draft, setDraft] = useState(() =>
		fixture ? architectureFromFixture(fixture) : newArchitecture("wall"),
	);
	const [snap, setSnap] = useState(true);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	async function save() {
		setBusy(true);
		setError(null);
		try {
			const snapshot = await documentSession.patchSnapshot();
			const next = snap
				? snapWallDraft(draft, snapshot.fixtures, fixture?.fixtureId)
				: draft;
			const extras = {
				name: next.name,
				sceneryOptions: {
					...fixture?.sceneryOptions,
					colourSrgb: next.colourSrgb,
					architecture: next.architecture,
				},
				scenerySizeMetres: next.size,
			};
			const rotation = {
				...(fixture?.rotation ?? { x: 0, y: 0, z: 0 }),
				z: next.angle,
			};
			if (fixture) {
				const edited = {
					...fixture,
					...extras,
					name: next.name,
					location: next.position,
					stagePositionSet: true,
					rotation,
				};
				const scene = await cadSession.snapshot();
				await cadSession.patchVenue(scene.sceneRevision, [edited]);
				onSaved?.(edited);
				onClose();
			} else {
				const result = await placeProfile(
					ARCHITECTURE_PROFILE,
					next.name,
					undefined,
					[{ position: next.position, rotation }],
					extras,
				);
				if (!result.ok) throw new Error(result.reason);
				onPlaced?.(result.fixtureIds);
			}
		} catch (reason) {
			setError(formatErrorDetails(reason));
		} finally {
			setBusy(false);
		}
	}
	return (
		<ModalFrame
			ariaLabel={fixture ? "Edit venue element" : "Venue tools"}
			title={fixture ? "Edit venue element" : "Venue tools"}
			dialogClassName="cad-architecture-modal"
			closeLabel="Close venue tools"
			onClose={onClose}
			accept={{
				id: "save-architecture",
				label: fixture ? "Apply" : "Place",
				disabled: busy,
				onPress: () => void save(),
			}}
		>
			<div className="cad-add-part-body" aria-busy={busy}>
				{error ? <ErrorAlert role="alert">{error}</ErrorAlert> : null}
				{!fixture ? (
					<label className="cad-field">
						Element
						<select
							className="ui-input"
							aria-label="Venue element type"
							value={draft.architecture.kind}
							onChange={(event) =>
								setDraft(
									newArchitecture(
										event.currentTarget.value as ArchitectureKind,
									),
								)
							}
						>
							{[
								"wall",
								"pillar",
								"slab",
								"stairs",
								"table",
								"chair",
								"bar",
							].map((value) => (
								<option key={value} value={value}>
									{value === "slab"
										? "Floor / roof / balcony"
										: value.charAt(0).toUpperCase() + value.slice(1)}
								</option>
							))}
						</select>
					</label>
				) : null}
				<fieldset disabled={busy}>
					<CadArchitectureFields draft={draft} onChange={setDraft} />
				</fieldset>
				{draft.architecture.kind === "wall" ? (
					<label className="cad-field">
						<input
							type="checkbox"
							checked={snap}
							onChange={(event) => setSnap(event.currentTarget.checked)}
						/>
						Snap endpoints to nearby walls (25 cm)
					</label>
				) : null}
			</div>
		</ModalFrame>
	);
}
