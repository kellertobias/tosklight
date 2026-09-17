// The Pixel Map dock: one output's picture with its display regions and pixel zones drawn on it,
// and the tables that edit them.
//
// It reads the same output configuration the picture settings do, and writes back only the map, so
// editing a zone cannot disturb the monitor or the frame rate the output is on.

import type { TitleActionGroup } from "@tosklight/ui/controls";
import { WindowFrame } from "@tosklight/ui/window-kit";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { useFailureToast } from "../../app/ToastContext";
import { ApiFailure, api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	OutputConfigurationView,
	OutputView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import { useOutputs } from "../../shared/api/queries";
import { PixelMapEditor } from "./PixelMapEditor";
import "./pixelMap.css";

export type PixelMapTab = "regions" | "zones";

export const PIXEL_MAP_TABS: { id: PixelMapTab; label: string }[] = [
	{ id: "regions", label: "Display Regions" },
	{ id: "zones", label: "Pixel Zones" },
];

/** The window every Pixel Map state is shown in, so the tabs never move while it loads. */
export function PixelMapFrame({
	tab,
	onTabChange,
	groups = [],
	children,
}: {
	tab: PixelMapTab;
	onTabChange: (tab: PixelMapTab) => void;
	groups?: TitleActionGroup[];
	children: ReactNode;
}) {
	return (
		<WindowFrame
			title="Pixel Map"
			info={{
				primary: "Media Server",
				secondary: "Display regions and pixel zones",
			}}
			className="media-pixel-map-window"
			groups={[
				{
					id: "pixel-map-tabs",
					kind: "tabs",
					activeId: tab,
					onActiveChange: (id) => onTabChange(id as PixelMapTab),
					actions: PIXEL_MAP_TABS.map((entry) => ({
						id: entry.id,
						label: entry.label,
					})),
				},
				...groups,
			]}
		>
			{children}
		</WindowFrame>
	);
}

export function PixelMapPage() {
	const outputs = useOutputs();
	const [tab, setTab] = useState<PixelMapTab>("regions");
	const [chosenId, setChosenId] = useState<string>();
	const list = outputs.data ?? [];
	const output = list.find((entry) => entry.id === chosenId) ?? list[0];

	if (!output) {
		return (
			<PixelMapFrame tab={tab} onTabChange={setTab}>
				<section className="media-page media-pixel-map-content">
					<ResourceState
						resource={outputs}
						subject="pixel mapping"
						isEmpty={(data) => data.length === 0}
						empty="No outputs are enabled."
					>
						{() => null}
					</ResourceState>
				</section>
			</PixelMapFrame>
		);
	}
	return (
		<OutputPixelMap
			key={output.id}
			output={output}
			outputs={list}
			onOutputChange={setChosenId}
			tab={tab}
			onTabChange={setTab}
		/>
	);
}

function OutputPixelMap({
	output,
	outputs,
	onOutputChange,
	tab,
	onTabChange,
}: {
	output: OutputView;
	outputs: OutputView[];
	onOutputChange: (id: string) => void;
	tab: PixelMapTab;
	onTabChange: (tab: PixelMapTab) => void;
}) {
	// The editor is keyed by the read that produced it, so a saved map reopens from the server's
	// copy rather than from the draft it replaced.
	const [loaded, setLoaded] = useState<{
		configuration: OutputConfigurationView;
		read: number;
	}>();
	const [failure, setFailure] = useState<ApiFailure>();
	const [revision, setRevision] = useState(0);
	const reload = useCallback(() => setRevision((current) => current + 1), []);
	const editing = useEditing(reload);
	useFailureToast(editing.failure);

	useEffect(() => {
		let current = true;
		void api
			.outputConfiguration(output.id)
			.then((configuration) => {
				if (current) setLoaded({ configuration, read: revision });
			})
			.catch((error: unknown) => {
				if (current && error instanceof ApiFailure) setFailure(error);
			});
		return () => {
			current = false;
		};
	}, [output.id, revision]);

	if (!loaded) {
		return (
			<PixelMapFrame tab={tab} onTabChange={onTabChange}>
				<section className="media-page media-pixel-map-content">
					{failure ? (
						<p className="media-state is-error" role="alert">
							The {output.name} pixel map could not be read. {failure.message}
						</p>
					) : (
						<p className="media-state" role="status">
							Reading {output.name}…
						</p>
					)}
				</section>
			</PixelMapFrame>
		);
	}
	const { configuration, read } = loaded;
	return (
		<PixelMapEditor
			key={`${configuration.id}-${read}`}
			output={configuration}
			outputs={outputs}
			onOutputChange={onOutputChange}
			tab={tab}
			onTabChange={onTabChange}
			busy={editing.busy}
			failed={editing.failure !== undefined}
			onSave={(pixelMap: PixelMapView) =>
				void editing.save(() =>
					api.updateOutputConfiguration(configuration.id, {
						requestId: requestId(),
						pixelMap,
					}),
				)
			}
		/>
	);
}
