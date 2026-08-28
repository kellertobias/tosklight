import { Button } from "@tosklight/ui";
import { CadDepthMenu } from "./CadDepthMenu";
import { type CadTileProps, fittedCamera } from "./CadApp";
import { CAD_VIEW_LABELS, type ViewportTile } from "./types";

/// A viewport's own controls: which way it looks, how it frames the rig, and the slice it shows.
///
/// The range settings take the same corner, so opening them slides the rest out rather than
/// stacking a second row of chrome over the drawing.
export function CadTileViewBar({
	node,
	scene,
	onTile,
	onFit,
	rangeOpen,
	setRangeOpen,
}: {
	node: ViewportTile;
	scene: CadTileProps["scene"];
	onTile: CadTileProps["onTile"];
	onFit: CadTileProps["onFit"];
	rangeOpen: boolean;
	setRangeOpen(open: boolean): void;
}) {
	return (
		<>
			<div
				className="cad-view-control"
				data-slid-out={rangeOpen ? "true" : "false"}
			>
				<select
					aria-label="View direction"
					value={node.view}
					onChange={(event) => {
						const view = event.currentTarget.value as ViewportTile["view"];
						onTile(node.id, (tile) => ({
							...tile,
							view,
							rotationQuarterTurns: 0,
							camera: fittedCamera(scene.entities, view, 0),
						}));
					}}
				>
					{Object.entries(CAD_VIEW_LABELS).map(([value, label]) => (
						<option key={value} value={value}>
							{label}
						</option>
					))}
				</select>
				<Button
					className="cad-fit-view"
					disabled={!scene.entities.length}
					onClick={() => onFit(node.id)}
				>
					Fit
				</Button>
				<Button
					className="cad-view-settings"
					aria-label="Range settings"
					title="Range settings"
					active={rangeOpen}
					onClick={() => setRangeOpen(true)}
				>
					<svg viewBox="0 0 16 16" aria-hidden="true">
						<circle cx="8" cy="8" r="2.4" />
						<path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" />
					</svg>
				</Button>
			</div>
			{rangeOpen ? (
				<CadDepthMenu
					view={node.view}
					entities={scene.entities}
					cutPlanes={node.cutPlanes}
					onChange={(c) =>
						onTile(node.id, (t) => ({ ...t, cutPlanes: c }))
					}
					onClose={() => setRangeOpen(false)}
				/>
			) : null}
		</>
	);
}
