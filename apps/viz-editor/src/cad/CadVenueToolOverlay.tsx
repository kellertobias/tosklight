import type { TileCamera } from "./types";
import type { VenueToolPreview } from "./useCadVenueTool";
import "./cadVenueDrawing.css";
export function CadVenueToolOverlay({
	preview,
	camera,
	active,
	message,
	error,
	busy,
	onDone,
}: {
	preview: VenueToolPreview | null;
	camera: TileCamera;
	active: boolean;
	message: string;
	error: string | null;
	busy: boolean;
	onDone(): void;
}) {
	if (!active) return null;
	return (
		<>
			<svg
				className="cad-venue-tool-preview"
				aria-label={
					preview
						? `${preview.kind} placement preview`
						: "Venue drawing preview"
				}
				data-venue-preview-kind={preview?.kind}
				data-snapped-wall={preview?.wallId}
				data-valid={preview?.valid === false ? "false" : "true"}
			>
				{preview ? (
					<polygon
						points={preview.points
							.map(
								([x, y]) =>
									`${(x + camera.pan[0]) * camera.zoom},${-(y + camera.pan[1]) * camera.zoom}`,
							)
							.join(" ")}
						transform="translate(0 0)"
					/>
				) : null}
			</svg>
			<div className="cad-venue-tool-status" role={error ? "alert" : "status"}>
				<span>
					{busy ? "Saving venue… " : ""}
					{message}
				</span>
				<button type="button" className="ui-button" onClick={onDone}>
					Done drawing
				</button>
			</div>
		</>
	);
}
