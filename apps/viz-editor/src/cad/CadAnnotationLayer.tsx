/**
 * The words of the items drawn on a viewport — placed text and each measurement's distance — and
 * the field text is typed into before it is placed.
 *
 * Like the fixture labels, they follow the camera in CSS rather than in the renderer, so they stay
 * crisp at any zoom. Placed text is sized in millimetres and grows with the plan; a measurement's
 * distance stays readable at any zoom.
 */
import type { CadAnnotation } from "./annotations";
import { annotationLabels } from "./annotationGeometry";
import type { PlanPoint } from "./projection";
import type { TileCamera } from "./types";

function at(point: PlanPoint, camera: TileCamera) {
	return {
		left: `calc(50% + ${(point[0] + camera.pan[0]) * camera.zoom}px)`,
		top: `calc(50% - ${(point[1] + camera.pan[1]) * camera.zoom}px)`,
	};
}

export function CadAnnotationLayer({
	annotations,
	rotationQuarterTurns,
	camera,
	pendingText,
	onCommitText,
	onCancelText,
}: {
	annotations: readonly CadAnnotation[];
	rotationQuarterTurns: number;
	camera: TileCamera;
	pendingText: PlanPoint | null;
	onCommitText(text: string): void;
	onCancelText(): void;
}) {
	const labels = annotationLabels(annotations, rotationQuarterTurns);
	if (!labels.length && !pendingText) return null;
	return (
		<div className="cad-annotation-labels">
			{labels.map((label) => (
				<span
					key={`${label.kind}:${label.id}`}
					className={`cad-annotation-label is-${label.kind}`}
					style={{
						...at(label.point, camera),
						...(label.heightMillimetres
							? { fontSize: `${label.heightMillimetres * camera.zoom}px` }
							: {}),
					}}
				>
					{label.text}
				</span>
			))}
			{pendingText ? (
				<input
					className="cad-annotation-text-input"
					aria-label="Text to place"
					placeholder="Type, then press Enter"
					// The operator clicked exactly to type here.
					// biome-ignore lint/a11y/noAutofocus: placing text is the explicit request
					autoFocus
					style={at(pendingText, camera)}
					onPointerDown={(event) => event.stopPropagation()}
					onKeyDown={(event) => {
						if (event.key === "Enter") onCommitText(event.currentTarget.value);
						if (event.key === "Escape") onCancelText();
					}}
				/>
			) : null}
		</div>
	);
}
