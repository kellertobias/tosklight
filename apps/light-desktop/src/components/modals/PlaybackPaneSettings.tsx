import { PlaybackPaneViewSettings } from "./PlaybackPaneViewSettings";
import { Button, ErrorAlert, SelectField } from "@tosklight/ui";
import { useState } from "react";
import { playbackSurfacePatch } from "../../features/screens/playbackSurfacePatch";
import { useScreens } from "../../features/screens/ScreensContext";
import { useApp } from "../../state/AppContext";
import type { PaneModel } from "../../types";
import { PlaybackLayoutModal } from "../setup/PlaybackLayoutModal";
import { copyPlaybackLayout, gridPlaybackLayout } from "@tosklight/ui/playback";

export function PlaybackPaneSettings({ pane }: { pane: PaneModel }) {
	const screens = useScreens();
	const { dispatch } = useApp();
	const id = pane.surface_id ?? `pane:${pane.id}`;
	const registry = screens.surfaces?.playback_surfaces ?? {};
	const configuration = registry[id] ?? {
		name: pane.title || "Playback pane",
		layout: gridPlaybackLayout(401, 20, 14),
		page_source: { type: "follow" as const, surface_id: "main" },
	};
	const [editing, setEditing] = useState(false);
	const [source, setSource] = useState("");
	const [draft, setDraft] = useState(configuration);
	const [error, setError] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	const save = async (next: typeof configuration) => {
		setSaving(true);
		setError(null);
		try {
			await screens.updatePlaybackSurface(
				id,
				playbackSurfacePatch(registry[id], next),
			);
			dispatch({
				type: "SET_PANE_PLAYBACK_SURFACE",
				id: pane.id,
				surfaceId: id,
			});
			setEditing(false);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setSaving(false);
		}
	};
	return (
		<section aria-label="Playback surface settings">
			<PlaybackPaneViewSettings pane={pane} />
			{(pane.playbackViewMode ?? "free-region") === "free-region" && <>
			<Button
				disabled={saving || !screens.surfaces}
				onClick={() => {
					setDraft(configuration);
					setEditing(true);
				}}
			>
				Configure layout and page source
			</Button>
			<SelectField
				label="Take from Screen"
				value={source}
				onChange={setSource}
				options={[
					{ value: "", label: "Choose a screen layout" },
					...(screens.screens?.screens ?? [])
						.filter((screen) => registry[screen.surface_id])
						.map((screen) => ({
							value: screen.surface_id,
							label: screen.name,
						})),
				]}
			/>
			<Button
				disabled={!source || saving}
				onClick={() => {
					const selected = registry[source];
					if (selected) {
						setDraft({
							...configuration,
							layout: copyPlaybackLayout(selected.layout),
						});
						setEditing(true);
					}
				}}
			>
				Review copied layout
			</Button>
			<p>
				Take from Screen copies the layout for review; playback assignments stay
				on their pages.
			</p>
			</>}
			{error && <ErrorAlert role="alert">{error}</ErrorAlert>}
			{editing && (
				<PlaybackLayoutModal
					acceptLabel="Apply"
					initialLayout={draft.layout}
					pageSource={draft.page_source}
					surfaceOptions={Object.entries(registry)
						.filter(([key]) => key !== id)
						.map(([key, surface]) => ({ id: key, name: surface.name }))}
					onClose={() => setEditing(false)}
					onSave={(layout, page_source) =>
						void save({ ...draft, layout, page_source })
					}
				/>
			)}
		</section>
	);
}
