import {
	Button,
	FormLayout,
	MultiValueToggleField,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
	TouchSelect,
} from "@tosklight/ui";
import {
	WindowSettings,
	type WindowSettingsTab,
} from "@tosklight/ui/window-kit";
import { useState } from "react";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeStatus,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import type { VirtualPlaybackZone } from "../../features/virtualPlaybackZones/contracts";
import { PRESET_FAMILIES } from "../../presetFamilies";
import { useApp } from "../../state/AppContext";
import {
	MAX_PLAYBACK_PAGE,
	MAX_VIRTUAL_PLAYBACK_CELLS,
} from "../../state/reducers/paneOptionsReducer";
import { GRID_COLUMNS, GRID_ROWS, type PaneModel } from "../../types";
import { useVirtualPlaybackSurfaceZones } from "../control/virtualPlayback/useVirtualPlaybackSurfaceZones";
import { PoolColorSettings } from "../shared/PoolColorSettings";
import { requestPaneRemoval } from "../shell/paneRemovalGuard";
import {
	type CuePaneCuelistPlayback,
	useCuePaneCuelistPlaybacks,
} from "./cuePaneCuelistAuthority";
import { VisualizationPaneSettings } from "./VisualizationPaneSettings";

function VirtualPlaybackZoneEditor({
	zone,
	zones,
	saving,
	persist,
	onEdit,
}: {
	zone: VirtualPlaybackZone;
	zones: readonly VirtualPlaybackZone[];
	saving: boolean;
	persist: (zones: readonly VirtualPlaybackZone[]) => void;
	onEdit: (zone: VirtualPlaybackZone) => void;
}) {
	const [name, setName] = useState(zone.name);
	const saveName = () => {
		const trimmed = name.trim();
		if (!trimmed || trimmed === zone.name) return;
		setName(trimmed);
		persist(
			zones.map((candidate) =>
				candidate.id === zone.id ? { ...candidate, name: trimmed } : candidate,
			),
		);
	};
	return (
		<article className="virtual-playback-zone-editor">
			<header>
				<TextField
					label={`Name for ${zone.name}`}
					maxLength={80}
					value={name}
					disabled={saving}
					onChange={(event) => setName(event.target.value)}
				/>
				<Button
					disabled={saving || !name.trim() || name.trim() === zone.name}
					onClick={saveName}
				>
					Save name
				</Button>
				<Button disabled={saving} onClick={() => onEdit(zone)}>
					Edit Zone
				</Button>
				<Button
					className="danger"
					disabled={saving}
					onClick={() =>
						persist(zones.filter((candidate) => candidate.id !== zone.id))
					}
				>
					Delete zone
				</Button>
			</header>
			<small>
				Virtual Playbacks {zone.playbackNumbers.join(", ")} · zone order{" "}
				{zones.findIndex((candidate) => candidate.id === zone.id) + 1}
			</small>
		</article>
	);
}

function VirtualPlaybackZoneSettings({
	pane,
	onEditZone,
}: {
	pane: PaneModel;
	onEditZone: (zone: VirtualPlaybackZone) => void;
}) {
	const desk = usePlaybackDeskView(true);
	const runtimeStatus = usePlaybackRuntimeStatus();
	const authorityReady = runtimeStatus.status === "ready" && desk !== null;
	const surface = useVirtualPlaybackSurfaceZones({
		surfaceId: pane.id,
		active: true,
		authorityReady,
		pageMode:
			(pane.virtualPlaybackPageMode ?? "follow_main") === "pinned"
				? {
						type: "pinned",
						page: pane.virtualPlaybackPinnedPage ?? 1,
					}
				: { type: "follow_main" },
	});
	return (
		<section
			className="virtual-playback-zone-settings"
			aria-label="Playback Exclusion Zones"
		>
			<p>
				Shift-select at least two cells in the pane to create a zone. A newly
				activated member releases the other active members; creating or editing
				a zone never operates a playback.
			</p>
			{surface.saving && <p role="status">Saving Playback Exclusion Zones…</p>}
			{!surface.ready ? (
				<p role={surface.error ? "alert" : "status"}>
					{surface.error ?? "Loading Playback Exclusion Zones…"}
				</p>
			) : surface.zones.length === 0 ? (
				<p>No exclusion zones are configured for this show.</p>
			) : (
				surface.zones.map((zone) => (
					<VirtualPlaybackZoneEditor
						key={zone.id}
						zone={zone}
						zones={surface.zones}
						saving={surface.saving}
						persist={(next) => void surface.persist(next)}
						onEdit={onEditZone}
					/>
				))
			)}
		</section>
	);
}

function CuePaneSettings({
	pane,
	cueLists,
}: {
	pane: PaneModel;
	cueLists: readonly CuePaneCuelistPlayback[];
}) {
	const { dispatch } = useApp();
	const fixedNumber = pane.fixedCueListNumber ?? cueLists[0]?.number;
	return (
		<FormLayout labelPlacement="side">
			<MultiValueToggleField
				label="Displayed Cuelist"
				value={pane.cueListSource ?? "fixed"}
				onChange={(source) =>
					dispatch({ type: "SET_PANE_CUELIST", id: pane.id, source })
				}
				options={[
					{ value: "fixed", label: "Fixed" },
					{ value: "follow-selection", label: "Follow selection" },
				]}
			/>
			<SelectField
				label="Cuelist"
				value={String(fixedNumber ?? "")}
				disabled={
					(pane.cueListSource ?? "fixed") !== "fixed" || cueLists.length === 0
				}
				onChange={(value) =>
					dispatch({
						type: "SET_PANE_CUELIST",
						id: pane.id,
						number: Number(value),
					})
				}
				options={cueLists.map((definition) => ({
					value: String(definition.number),
					label: `${definition.number} · ${definition.name}`,
				}))}
			/>
			<SwitchField
				label="Compact Cue rows"
				offLabel="Standard"
				onLabel="Compact"
				checked={pane.cueListCompactRows ?? false}
				onChange={(event) =>
					dispatch({
						type: "SET_PANE_CUELIST_COMPACT_ROWS",
						id: pane.id,
						value: event.target.checked,
					})
				}
			/>
		</FormLayout>
	);
}

function PaneLayoutSettings({
	pane,
	maximized,
}: {
	pane: PaneModel;
	maximized: boolean;
}) {
	const { dispatch } = useApp();
	return (
		<>
			<p>
				Selected pane: <b>{pane.title}</b>
			</p>
			<div className="size-grid">
				<TouchSelect
					label="Grid column"
					value={pane.x}
					options={Array.from(
						{ length: GRID_COLUMNS },
						(_, index) => index + 1,
					)}
					onChange={(x) =>
						dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { x } })
					}
				/>
				<TouchSelect
					label="Grid row"
					value={pane.y}
					options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)}
					onChange={(y) =>
						dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { y } })
					}
				/>
				<TouchSelect
					label="Grid width"
					value={pane.width}
					options={Array.from(
						{ length: GRID_COLUMNS },
						(_, index) => index + 1,
					)}
					onChange={(width) =>
						dispatch({
							type: "SET_PANE_RECT",
							id: pane.id,
							rect: { width },
						})
					}
				/>
				<TouchSelect
					label="Grid height"
					value={pane.height}
					options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)}
					onChange={(height) =>
						dispatch({
							type: "SET_PANE_RECT",
							id: pane.id,
							rect: { height },
						})
					}
				/>
			</div>
			<div className="dialog-grid">
				<Button
					onClick={() => dispatch({ type: "TOGGLE_MAXIMIZE", id: pane.id })}
				>
					{maximized ? "Restore pane" : "Maximize pane"}
				</Button>
				<Button
					className="danger"
					onClick={() => {
						if (requestPaneRemoval(pane.id))
							dispatch({ type: "REMOVE_PANE", id: pane.id });
					}}
				>
					Remove pane
				</Button>
			</div>
		</>
	);
}

function StagePaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	const setOption = (
		option:
			| "stageView"
			| "followPreload"
			| "stage2dSide",
		value:
			| boolean
			| NonNullable<PaneModel["stageView"]>
			| NonNullable<PaneModel["stage2dSide"]>,
	) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option, value });
	return (
		<FormLayout labelPlacement="side">
			<MultiValueToggleField
				label="Stage view"
				value={pane.stageView ?? "2d"}
				onChange={(value) => setOption("stageView", value)}
				options={[
					{ value: "2d", label: "2D" },
					{ value: "3d", label: "3D" },
					{ value: "3d-viz", label: "3D Viz" },
				]}
			/>
			<SwitchField
				label="Preload source"
				offLabel="Manual"
				onLabel="Follow preload"
				checked={Boolean(pane.followPreload)}
				onChange={(event) => setOption("followPreload", event.target.checked)}
			/>
			{/*
			 * A 2D Stage is the renderer's plan of the rig, so the only thing it can be asked is
			 * which side to look from. What is in a beam belongs to the view and to the render
			 * quality, neither of which is a per-pane choice.
			 */}
			{(pane.stageView ?? "2d") === "2d" && (
				<SelectField
					label="Viewed from"
					value={pane.stage2dSide ?? "top"}
					onChange={(value) => setOption("stage2dSide", value)}
					options={[
						{ value: "top", label: "Above · plan" },
						{ value: "front", label: "Front · from the house" },
						{ value: "back", label: "Back · from upstage" },
						{ value: "left", label: "House left · stage right" },
						{ value: "right", label: "House right · stage left" },
					]}
				/>
			)}
		</FormLayout>
	);
}

function ChannelPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<FormLayout labelPlacement="side">
			<MultiValueToggleField
				label="Displayed channels"
				value={pane.channelDisplayMode ?? "intensity"}
				onChange={(mode) =>
					dispatch({
						type: "SET_PANE_CHANNEL_DISPLAY_MODE",
						id: pane.id,
						mode,
					})
				}
				options={[
					{ value: "intensity", label: "Intensity only" },
					{ value: "all", label: "All channels" },
				]}
			/>
		</FormLayout>
	);
}

export function RunningPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<FormLayout labelPlacement="side">
			<MultiValueToggleField
				label="Running kind"
				value={pane.runningFilter ?? "all"}
				onChange={(filter) =>
					dispatch({
						type: "SET_PANE_RUNNING_FILTER",
						id: pane.id,
						filter,
					})
				}
				options={[
					{ value: "all", label: "All" },
					{ value: "cue_list", label: "Cuelists" },
					{ value: "dynamic", label: "Dynamics" },
					{ value: "timecode", label: "Timecodes" },
					{ value: "macro", label: "Macros" },
				]}
			/>
		</FormLayout>
	);
}

function VirtualPlaybackGridSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	const rows = pane.virtualPlaybackRows ?? 2;
	const columns = pane.virtualPlaybackColumns ?? 2;
	const pageMode = pane.virtualPlaybackPageMode ?? "follow_main";
	const pinnedPage = pane.virtualPlaybackPinnedPage ?? 1;
	return (
		<>
			<FormLayout labelPlacement="side">
				<NumberField
					label="Rows"
					min="1"
					max={String(MAX_VIRTUAL_PLAYBACK_CELLS)}
					value={rows}
					onChange={(event) =>
						dispatch({
							type: "SET_VIRTUAL_PLAYBACK_GRID",
							id: pane.id,
							rows: Number(event.target.value),
							columns,
							changed: "rows",
						})
					}
				/>
				<NumberField
					label="Columns"
					min="1"
					max={String(MAX_VIRTUAL_PLAYBACK_CELLS)}
					value={columns}
					onChange={(event) =>
						dispatch({
							type: "SET_VIRTUAL_PLAYBACK_GRID",
							id: pane.id,
							rows,
							columns: Number(event.target.value),
							changed: "columns",
						})
					}
				/>
				<MultiValueToggleField
					label="Page mode"
					value={pageMode}
					onChange={(mode) =>
						dispatch({
							type: "SET_VIRTUAL_PLAYBACK_PAGE_MODE",
							id: pane.id,
							mode,
							pinnedPage,
						})
					}
					options={[
						{ value: "follow_main", label: "Follow Main" },
						{ value: "pinned", label: "Pinned" },
					]}
				/>
				{pageMode === "pinned" && (
					<NumberField
						label="Pinned page"
						min="1"
						max={String(MAX_PLAYBACK_PAGE)}
						value={pinnedPage}
						onChange={(event) =>
							dispatch({
								type: "SET_VIRTUAL_PLAYBACK_PAGE_MODE",
								id: pane.id,
								mode: "pinned",
								pinnedPage: Number(event.target.value),
							})
						}
					/>
				)}
			</FormLayout>
			<p role="status">
				{(rows * columns).toLocaleString()} of{" "}
				{MAX_VIRTUAL_PLAYBACK_CELLS.toLocaleString()} available Virtual Playback
				positions.
			</p>
		</>
	);
}

function PresetPoolPaneSettings({ pane }: { pane: PaneModel }) {
	const { state, dispatch } = useApp();
	const family = pane.presetFamily ?? state.presetFamily;
	return (
		<>
			<h3>Preset family</h3>
			<div className="button-group">
				{PRESET_FAMILIES.map((candidate) => (
					<Button
						key={candidate}
						className={family === candidate ? "active" : ""}
						onClick={() =>
							dispatch({
								type: "SET_PANE_PRESET_FAMILY",
								id: pane.id,
								family: candidate,
							})
						}
					>
						{candidate}
					</Button>
				))}
			</div>
			<PoolColorSettings
				objectType="preset"
				paneId={pane.id}
				presetFamily={family.toLowerCase() as Lowercase<typeof family>}
				legacyPresetColors={pane.presetPoolColors ?? true}
			/>
		</>
	);
}

function GroupPoolPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<>
			<NumberField
				label="Columns"
				min="1"
				max="24"
				value={pane.poolColumns ?? 4}
				onChange={(event) =>
					dispatch({
						type: "SET_PANE_POOL_COLUMNS",
						id: pane.id,
						value: Number(event.target.value),
					})
				}
			/>
			<PoolColorSettings objectType="group" paneId={pane.id} />
		</>
	);
}

function paneLayoutTab(pane: PaneModel, maximized: boolean): WindowSettingsTab {
	return {
		id: "pane",
		label: "Pane Settings",
		content: <PaneLayoutSettings pane={pane} maximized={maximized} />,
	};
}

function FileManagerPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<FormLayout labelPlacement="side">
			<SwitchField
				label="Hidden files"
				offLabel="Hidden"
				onLabel="Visible"
				checked={Boolean(pane.fileManagerShowHidden)}
				onChange={(event) =>
					dispatch({
						type: "SET_FILE_MANAGER_SHOW_HIDDEN",
						id: pane.id,
						value: event.target.checked,
					})
				}
			/>
		</FormLayout>
	);
}

function TextEditorPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<FormLayout labelPlacement="side">
			<SwitchField
				label="Editing"
				offLabel="Editable"
				onLabel="Read only"
				checked={Boolean(pane.textEditorReadOnly)}
				onChange={(event) =>
					dispatch({
						type: "SET_TEXT_EDITOR_SETTINGS",
						id: pane.id,
						readOnly: event.target.checked,
					})
				}
			/>
			<MultiValueToggleField
				label="View"
				value={pane.textEditorMode ?? "plain"}
				onChange={(mode) =>
					dispatch({ type: "SET_TEXT_EDITOR_SETTINGS", id: pane.id, mode })
				}
				options={[
					{ value: "plain", label: "Plain Text" },
					{ value: "markdown", label: "Rendered Markdown" },
					{ value: "split", label: "Edit + Markdown" },
				]}
			/>
		</FormLayout>
	);
}

function PaneGroupShortcutsSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<SwitchField
			label="Group shortcuts"
			offLabel="Hidden"
			onLabel="Visible"
			checked={Boolean(pane.showGroupShortcuts)}
			onChange={(event) =>
				dispatch({
					type: "SET_PANE_GROUP_SHORTCUTS",
					id: pane.id,
					value: event.target.checked,
				})
			}
		/>
	);
}

function FixtureSheetPaneSettings({ pane }: { pane: PaneModel }) {
	const { dispatch } = useApp();
	return (
		<FormLayout>
			<SelectField
				label="Compact mode"
				value={pane.fixtureSheetCompactMode ?? "off"}
				onChange={(mode) =>
					dispatch({
						type: "SET_PANE_FIXTURE_COMPACT_MODE",
						id: pane.id,
						mode,
					})
				}
				options={[
					{ value: "off", label: "Off" },
					{ value: "icon-only", label: "Icon only" },
					{ value: "text-only", label: "Text only" },
				]}
			/>
			<SwitchField
				label="Show active fixtures only"
				offLabel="All fixtures"
				onLabel="Programmer only"
				checked={Boolean(pane.fixtureSheetActiveOnly)}
				onChange={(event) =>
					dispatch({
						type: "SET_PANE_FIXTURE_ACTIVE_ONLY",
						id: pane.id,
						value: event.target.checked,
					})
				}
			/>
		</FormLayout>
	);
}

function VirtualPlaybackExclusionSettings({
	pane,
	close,
}: {
	pane: PaneModel;
	close: () => void;
}) {
	const { dispatch } = useApp();
	return (
		<VirtualPlaybackZoneSettings
			pane={pane}
			onEditZone={(zone) => {
				dispatch({
					type: "SET_VIRTUAL_PLAYBACK_ZONE_EDIT",
					edit: {
						zoneId: zone.id,
						name: zone.name,
						playbackNumbers: [...zone.playbackNumbers],
					},
				});
				close();
			}}
		/>
	);
}

function paneSpecificTabs(
	pane: PaneModel,
	cuePaneCueLists: readonly CuePaneCuelistPlayback[],
	close: () => void,
) {
	const tabs: WindowSettingsTab[] = [];
	if (pane.kind === "cues")
		tabs.push({
			id: "cues",
			label: "Cues",
			content: <CuePaneSettings pane={pane} cueLists={cuePaneCueLists} />,
		});
	const poolType =
		pane.kind === "presets"
			? "preset"
			: pane.kind === "groups"
				? "group"
				: ["cuelists", "cuelist_pool", "qlists", "qlist_pool"].includes(
							pane.kind,
						)
					? "cuelist"
					: pane.kind === "dynamics"
						? "dynamic"
						: null;
	if (poolType)
		tabs.push({
			id: "pool",
			label: "Pool",
			content:
				poolType === "preset" ? (
					<PresetPoolPaneSettings pane={pane} />
				) : poolType === "group" ? (
					<GroupPoolPaneSettings pane={pane} />
				) : (
					<PoolColorSettings objectType={poolType} paneId={pane.id} />
				),
		});
	if (pane.kind === "stage")
		tabs.push({
			id: "stage",
			label: "Stage",
			content: <StagePaneSettings pane={pane} />,
		});
	if (pane.kind === "channels")
		tabs.push({
			id: "channels",
			label: "Channels",
			content: <ChannelPaneSettings pane={pane} />,
		});
	if (pane.kind === "running")
		tabs.push({
			id: "running",
			label: "Running",
			content: <RunningPaneSettings pane={pane} />,
		});
	if (pane.kind === "visualization")
		tabs.push({
			id: "visualization",
			label: "Visualization",
			content: <VisualizationPaneSettings pane={pane} />,
		});
	if (pane.kind === "fixtures")
		tabs.push({
			id: "fixture-sheet",
			label: "Fixture Sheet",
			content: <FixtureSheetPaneSettings pane={pane} />,
		});
	if (pane.kind === "virtual_playbacks")
		tabs.push(
			{
				id: "virtual",
				label: "Virtual Playbacks",
				content: <VirtualPlaybackGridSettings pane={pane} />,
			},
			{
				id: "virtual-exclusion-zones",
				label: "Exclusion Zones",
				content: <VirtualPlaybackExclusionSettings pane={pane} close={close} />,
			},
		);
	if (pane.kind === "file_manager")
		tabs.push({
			id: "files",
			label: "File Manager",
			content: <FileManagerPaneSettings pane={pane} />,
		});
	if (pane.kind === "text_editor")
		tabs.push({
			id: "editor",
			label: "Text Editor",
			content: <TextEditorPaneSettings pane={pane} />,
		});
	if (["stage", "fixtures", "presets"].includes(pane.kind))
		tabs.push({
			id: "shortcuts",
			label: "Shortcuts",
			content: <PaneGroupShortcutsSettings pane={pane} />,
		});
	return tabs;
}

export function PaneSettingsModal() {
	const { state } = useApp();
	if (!state.paneSettingsId) return null;
	const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
	const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
	if (!pane) return null;
	return <PaneSettingsDialog pane={pane} />;
}

function PaneSettingsDialog({ pane }: { pane: PaneModel }) {
	const { state, dispatch } = useApp();
	const cuePaneCueLists = useCuePaneCuelistPlaybacks(pane.kind === "cues");
	const close = () => dispatch({ type: "SET_PANE_SETTINGS", id: null });
	const maximized = state.maximizedPaneId === pane.id;
	const tabs = [
		paneLayoutTab(pane, maximized),
		...paneSpecificTabs(pane, cuePaneCueLists, close),
	];
	return <WindowSettings title="Pane Settings" tabs={tabs} onClose={close} />;
}
