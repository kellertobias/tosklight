/**
 * The DOM drawn over a CAD viewport's canvas: the scale bar an operator reads a distance off, and
 * the fixture identities beside the rig.
 *
 * Both follow the same camera the canvas does, in CSS rather than in the renderer, so text stays
 * crisp at any zoom and is selectable by a screen reader rather than baked into pixels.
 */
import type { CadViewportScale } from "./CadViewport";
import type {
	CadEntity,
	CadTransformPreview,
	CadViewDirection,
	TileCamera,
} from "./types";
import { previewDeltaForEntity, projectPoint } from "./types";

/** The measured rule in the viewport corner. */
export function CadScaleBar({
	scale,
	printMode,
}: {
	scale: CadViewportScale;
	printMode: boolean;
}) {
	return (
		<div
			className={`cad-viewport-scale ${printMode ? "is-print-mode" : ""}`.trim()}
			aria-label={`Scale ${scale.label}`}
			style={{ width: `${scale.pixelWidth}px` }}
		>
			<span className="cad-viewport-scale-label">{scale.label}</span>
			<span className="cad-viewport-scale-rule" aria-hidden="true">
				<span className="cad-viewport-scale-tick is-start" />
				<span className="cad-viewport-scale-tick is-quarter" />
				<span className="cad-viewport-scale-tick is-middle" />
				<span className="cad-viewport-scale-tick is-three-quarter" />
				<span className="cad-viewport-scale-tick is-end" />
			</span>
		</div>
	);
}

/** Fixture IDs and DMX patch beside each fixture, following a move while it is previewed. */
export function CadEntityLabels({
	entities,
	preview,
	view,
	rotationQuarterTurns,
	camera,
	showFixtureIds,
	showDmxAddresses,
}: {
	entities: readonly CadEntity[];
	preview: CadTransformPreview | null;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
}) {
	if (!showFixtureIds && !showDmxAddresses) return null;
	return (
		<div className="cad-entity-labels" aria-hidden="true">
			{entities.map((entity) => {
				const worldDelta = previewDeltaForEntity(
					preview,
					entity.logicalFixtureId,
				);
				const point = projectPoint(
					entity.positionMillimetres.map(
						(value, index) => value + worldDelta[index],
					) as [number, number, number],
					view,
					rotationQuarterTurns,
				);
				return (
					<span
						key={entity.id}
						className="cad-entity-label"
						style={{
							left: `calc(50% + ${(point[0] + camera.pan[0]) * camera.zoom}px)`,
							top: `calc(50% - ${(point[1] + camera.pan[1]) * camera.zoom}px)`,
						}}
					>
						{showFixtureIds ? `ID ${entity.fixtureDisplayId}` : null}
						{showFixtureIds && showDmxAddresses ? " · " : null}
						{showDmxAddresses ? `DMX ${entity.dmxAddress}` : null}
					</span>
				);
			})}
		</div>
	);
}
