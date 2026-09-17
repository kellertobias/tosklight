// One output's pixel map, edited as a draft and saved as a whole.
//
// The picture and the open table share one selection: pressing a shape on the picture selects its
// row, and selecting a row marks its shape, so an operator always knows which rectangle a row is.

import { Button, SelectField } from "@tosklight/ui/controls";
import { useState } from "react";
import { useDeskShowName } from "../../operator/DeskIdentityContext";
import type {
	DisplayRegionView,
	OutputConfigurationView,
	OutputView,
	PixelMapView,
	PixelZoneView,
} from "../../shared/api/generated/media-wire";
import { PixelHandoffEditor } from "./PixelHandoffEditor";
import { PixelMapFrame, type PixelMapTab } from "./PixelMapPage";
import { PixelMapPicture } from "./PixelMapPicture";
import { PixelRegionTable } from "./PixelRegionTable";
import { PixelRouteTable, PixelZoneTable } from "./PixelZoneTable";
import { replaceById } from "./pixelMapCells";
import {
	newHandoff,
	newRegion,
	newRoute,
	newZone,
	pixelMapProblems,
} from "./pixelMapEditing";

function withMode(map: PixelMapView, mode: PixelMapView["mode"]): PixelMapView {
	return {
		...map,
		mode,
		handoffs:
			mode === "direct"
				? []
				: map.zones.map(
						(zone) =>
							map.handoffs.find((handoff) => handoff.zoneId === zone.id) ??
							newHandoff(zone),
					),
	};
}

function saveStateLabel(busy: boolean, failed: boolean, dirty: boolean) {
	if (busy) return "Saving…";
	if (failed) return "Not saved · Check the error";
	if (dirty) return "Unsaved changes · Applies on restart";
	return "Saved · Applies on restart";
}

/** The draft map and the two selections, with the edits the tables and title actions make. */
function usePixelMapDraft(initial: PixelMapView) {
	const [map, setMap] = useState<PixelMapView>(initial);
	const [selectedZoneId, setSelectedZoneId] = useState<string | null>(
		initial.zones[0]?.id ?? null,
	);
	const [selectedRegionId, setSelectedRegionId] = useState<string | null>(
		initial.regions[0]?.id ?? null,
	);
	const addZone = () => {
		const zone = newZone(map.zones);
		setMap((current) => ({
			...current,
			zones: [...current.zones, zone],
			handoffs:
				current.mode === "desk-merge"
					? [...current.handoffs, newHandoff(zone)]
					: current.handoffs,
		}));
		setSelectedZoneId(zone.id);
	};
	const addRegion = () => {
		const region = newRegion(map.regions);
		setMap((current) => ({
			...current,
			regions: [...current.regions, region],
		}));
		setSelectedRegionId(region.id);
	};
	const replaceZone = (zone: PixelZoneView) => {
		setMap((current) => ({
			...current,
			zones: replaceById(current.zones, zone),
		}));
		setSelectedZoneId(zone.id);
	};
	const replaceRegion = (region: DisplayRegionView) => {
		setMap((current) => ({
			...current,
			regions: replaceById(current.regions, region),
		}));
		setSelectedRegionId(region.id);
	};
	const removeZone = (id: string) => {
		setMap((current) => ({
			...current,
			zones: current.zones.filter((zone) => zone.id !== id),
			handoffs: current.handoffs.filter((handoff) => handoff.zoneId !== id),
		}));
		setSelectedZoneId((current) => (current === id ? null : current));
	};
	const removeRegion = (id: string) => {
		setMap((current) => ({
			...current,
			regions: current.regions.filter((region) => region.id !== id),
		}));
		setSelectedRegionId((current) => (current === id ? null : current));
	};
	return {
		map,
		setMap,
		selectedZoneId,
		setSelectedZoneId,
		selectedRegionId,
		setSelectedRegionId,
		addZone,
		addRegion,
		replaceZone,
		replaceRegion,
		removeZone,
		removeRegion,
	};
}

type PixelMapDraft = ReturnType<typeof usePixelMapDraft>;

function ZonesPanel({ draft }: { draft: PixelMapDraft }) {
	const deskShowName = useDeskShowName();
	const { map, setMap, selectedZoneId } = draft;
	const selectedZone = map.zones.find((zone) => zone.id === selectedZoneId);
	const selectedHandoff = map.handoffs.find(
		(handoff) => handoff.zoneId === selectedZoneId,
	);
	return (
		<>
			<section aria-label="Pixel zones">
				<SelectField
					label="Operating mode"
					labelPlacement="side"
					description="Direct sends Media Server pixels. Desk merge combines a desk fixture with Media Server pixels and sends the result."
					value={map.mode}
					options={[
						{ value: "direct", label: "Direct Media Server output" },
						{ value: "desk-merge", label: "Desk merge" },
					]}
					onChange={(mode) => setMap((current) => withMode(current, mode))}
				/>
				<PixelZoneTable
					zones={map.zones}
					selectedId={selectedZoneId}
					onSelect={draft.setSelectedZoneId}
					onChange={draft.replaceZone}
					onRemove={draft.removeZone}
				/>
			</section>
			{map.mode === "desk-merge" && selectedZone && selectedHandoff && (
				<PixelHandoffEditor
					handoff={selectedHandoff}
					zone={selectedZone}
					deskShowName={deskShowName}
					onChange={(next) =>
						setMap((current) => ({
							...current,
							handoffs: current.handoffs.map((candidate) =>
								candidate.zoneId === next.zoneId ? next : candidate,
							),
						}))
					}
				/>
			)}
			<section aria-label="Media Server DMX output">
				<div className="media-pixel-map-subheading">
					<h3>Output routes</h3>
					<Button
						size="compact"
						onClick={() =>
							setMap((current) => ({
								...current,
								routes: [...current.routes, newRoute(current.routes)],
							}))
						}
					>
						Add output route
					</Button>
				</div>
				<PixelRouteTable
					routes={map.routes}
					onChange={(route) =>
						setMap((current) => ({
							...current,
							routes: replaceById(current.routes, route),
						}))
					}
					onRemove={(id) =>
						setMap((current) => ({
							...current,
							routes: current.routes.filter((route) => route.id !== id),
						}))
					}
				/>
			</section>
		</>
	);
}

export function PixelMapEditor({
	output,
	outputs,
	onOutputChange,
	tab,
	onTabChange,
	busy,
	failed,
	onSave,
}: {
	output: OutputConfigurationView;
	outputs: OutputView[];
	onOutputChange: (id: string) => void;
	tab: PixelMapTab;
	onTabChange: (tab: PixelMapTab) => void;
	busy: boolean;
	failed: boolean;
	onSave: (map: PixelMapView) => void;
}) {
	const draft = usePixelMapDraft(output.pixelMap);
	const { map } = draft;
	const problems = pixelMapProblems(map);
	const dirty = map !== output.pixelMap;
	const addAction =
		tab === "regions"
			? {
					id: "add-region",
					label: "Add display region",
					onPress: draft.addRegion,
				}
			: { id: "add-zone", label: "Add pixel zone", onPress: draft.addZone };

	return (
		<PixelMapFrame
			tab={tab}
			onTabChange={onTabChange}
			groups={[
				{
					id: "pixel-map-actions",
					actions: [
						addAction,
						{
							id: "save",
							label: "Save pixel map",
							variant: "primary",
							disabled: busy || problems.length > 0 || !dirty,
							onPress: () => onSave(map),
						},
					],
				},
			]}
		>
			<section className="media-page media-pixel-map-content">
				<div className="media-pixel-map-toolbar">
					{outputs.length > 1 && (
						<SelectField
							label="Output"
							labelPlacement="side"
							value={output.id}
							options={outputs.map((entry) => ({
								value: entry.id,
								label: entry.name,
							}))}
							onChange={onOutputChange}
						/>
					)}
					<p
						className="media-settings-save-state"
						role="status"
						aria-live="polite"
					>
						{saveStateLabel(busy, failed, dirty)}
					</p>
				</div>
				<div className="media-pixel-map-layout">
					<PixelMapPicture
						output={output}
						map={map}
						tab={tab}
						selectedRegionId={draft.selectedRegionId}
						selectedZoneId={draft.selectedZoneId}
						onSelectRegion={draft.setSelectedRegionId}
						onSelectZone={draft.setSelectedZoneId}
					/>
					<div className="media-pixel-map-tables">
						{tab === "regions" ? (
							<section aria-label="Display regions">
								<PixelRegionTable
									regions={map.regions}
									selectedId={draft.selectedRegionId}
									onSelect={draft.setSelectedRegionId}
									onChange={draft.replaceRegion}
									onRemove={draft.removeRegion}
								/>
							</section>
						) : (
							<ZonesPanel draft={draft} />
						)}
						{problems.length > 0 && (
							<ul
								className="media-settings-problems"
								aria-label="Pixel map problems"
							>
								{problems.map((problem) => (
									<li key={problem}>{problem}</li>
								))}
							</ul>
						)}
					</div>
				</div>
			</section>
		</PixelMapFrame>
	);
}
