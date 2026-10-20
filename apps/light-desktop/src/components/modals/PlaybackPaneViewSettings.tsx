import {
	ErrorAlert,
	MultiValueToggleField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import { gridPlaybackLayout } from "@tosklight/ui/playback";
import { useState } from "react";
import { useScreens } from "../../features/screens/ScreensContext";
import { useApp } from "../../state/AppContext";
import type { PaneModel } from "../../types";

/** A pane mirrors screen geometry but retains its own page/held-control identity. */
function usePlaybackPaneView(pane: PaneModel) {
	const screens = useScreens();
	const { dispatch } = useApp();
	const screenIds = [
		"main",
		...(screens.screens?.screens ?? []).map((screen) => screen.surface_id),
	];
	const id =
		pane.surface_id && !screenIds.includes(pane.surface_id)
			? pane.surface_id
			: `pane:${pane.id}`;
	const registry = screens.surfaces?.playback_surfaces ?? {};
	const mode = pane.playbackViewMode ?? "free-region";
	const selected = pane.playbackScreenSurfaceId ?? "main";
	const configuration = registry[id];
	const source = configuration?.page_source ?? {
		type: "follow" as const,
		surface_id: "main",
	};
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const screenOptions = [
		{ value: "main", label: registry.main?.name ?? "Main" },
		...(screens.screens?.screens ?? []).map((screen) => ({
			value: screen.surface_id,
			label: screen.name,
		})),
	];
	const change = async (
		nextMode: "screen" | "free-region",
		screenSurfaceId: string,
		pageSource = source,
	) => {
		setSaving(true);
		setError(null);
		try {
			await screens.updatePlaybackSurface(
				id,
				configuration
					? { page_source: pageSource }
					: {
							name: pane.title || "Playbacks",
							layout: gridPlaybackLayout(401, 20, 14),
							page_source: pageSource,
						},
			);
			dispatch({
				type: "SET_PANE_PLAYBACK_SURFACE",
				id: pane.id,
				surfaceId: id,
			});
			dispatch({
				type: "SET_PANE_PLAYBACK_VIEW",
				id: pane.id,
				mode: nextMode,
				screenSurfaceId,
			});
			if (nextMode === "screen")
				dispatch({
					type: "SET_PANE_TILE_LAYOUT",
					id: pane.id,
					layout: undefined,
				});
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		} finally {
			setSaving(false);
		}
	};
	const pageMode =
		source.type !== "follow"
			? "custom"
			: source.surface_id === selected
				? "screen"
				: "other";
	return {
		screens,
		dispatch,
		id,
		mode,
		selected,
		source,
		saving,
		error,
		screenOptions,
		change,
		pageMode,
	};
}

export function PlaybackPaneViewSettings({ pane }: { pane: PaneModel }) {
	const {
		screens,
		dispatch,
		id,
		mode,
		selected,
		source,
		saving,
		error,
		screenOptions,
		change,
		pageMode,
	} = usePlaybackPaneView(pane);
	return (
		<>
			<SwitchField
				label="Show playback controls"
				offLabel="Tiles"
				onLabel="Controls"
				checked={pane.playbackShowControls ?? false}
				onChange={(event) =>
					dispatch({
						type: "SET_PANE_PLAYBACK_CONTROLS",
						id: pane.id,
						showControls: event.target.checked,
					})
				}
			/>
			<MultiValueToggleField
				label="Displayed region"
				value={mode}
				options={[
					{ value: "screen", label: "Screen" },
					{ value: "free-region", label: "Free region" },
				]}
				disabled={saving || !screens.surfaces}
				onChange={(value) =>
					void change(
						value,
						selected,
						value === "screen"
							? { type: "follow", surface_id: selected }
							: source,
					)
				}
			/>
			{mode === "screen" && (
				<SelectField
					ariaLabel="Screen"
					label="Screen"
					value={selected}
					options={screenOptions}
					disabled={saving}
					onChange={(value) =>
						void change(
							mode,
							value,
							pageMode === "screen"
								? { type: "follow", surface_id: value }
								: source,
						)
					}
				/>
			)}
			<SelectField
				ariaLabel="Page source"
				label="Page source"
				value={pageMode}
				disabled={saving || !screens.surfaces}
				options={[
					{ value: "screen", label: "Follow selected screen" },
					{ value: "other", label: "Follow another screen" },
					{ value: "custom", label: "Custom page" },
				]}
				onChange={(value) =>
					void change(
						mode,
						selected,
						value === "custom"
							? { type: "own", page: screens.surfaces?.resolved_pages[id] ?? 1 }
							: {
									type: "follow",
									surface_id:
										value === "screen"
											? selected
											: (screenOptions.find((item) => item.value !== selected)
													?.value ?? "main"),
								},
					)
				}
			/>
			{pageMode === "other" && (
				<SelectField
					ariaLabel="Follow screen"
					label="Follow screen"
					value={source.type === "follow" ? source.surface_id : "main"}
					options={screenOptions}
					disabled={saving}
					onChange={(value) =>
						void change(mode, selected, { type: "follow", surface_id: value })
					}
				/>
			)}
			{saving && <p role="status">Saving Playback view…</p>}
			{error && <ErrorAlert role="alert">{error}</ErrorAlert>}
		</>
	);
}
