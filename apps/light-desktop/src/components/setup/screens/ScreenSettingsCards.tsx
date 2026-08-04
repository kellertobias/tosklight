import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type {
	FixedScreenPane,
	PlaybackSurfaceLayout,
	ScreenConfiguration,
} from "../../../api/types";
import type { DeskModel } from "../../../types";
import { PlaybackLayoutModal } from "../PlaybackLayoutModal";
import {
	DEFAULT_FIXED_SCREEN_PANE,
	DEFAULT_FIXED_SIDE_WIDTH_PX,
	browserScreenUrl,
	playbackLayoutLegacyFields,
	screenPlaybackLayout,
	updateScreenConfiguration,
} from "../screenConfiguration";

type CuelistOption = { id: string; name: string };
type TextFileOption = {
	root: string;
	rootLabel: string;
	path: string;
	name: string;
};

const fixedPaneLabels: Record<FixedScreenPane["type"], string> = {
	fixture_sheet: "Fixture Sheet",
	stage_2d: "Stage - 2D",
	stage_3d: "Stage - 3D",
	cues: "Cues - Cuelist",
	text: "Text",
};

function defaultFixedPane(type: FixedScreenPane["type"]): FixedScreenPane {
	switch (type) {
		case "fixture_sheet":
			return DEFAULT_FIXED_SCREEN_PANE;
		case "stage_2d":
			return { type, follow_preload: false, show_floor_grid: true };
		case "stage_3d":
			return {
				type,
				follow_preload: false,
				show_floor_grid: true,
				show_beam_guides: true,
				render_quality: "lines_and_beams",
				environment_brightness: 1,
			};
		case "cues":
			return { type, cue_list_id: "" };
		case "text":
			return { type, root: "", path: "", mode: "plain" };
	}
}

function FixtureSheetFixedSettings({
	pane,
	cueLists,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "fixture_sheet" }>;
	cueLists: readonly CuelistOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	const columnOptions = [
		["id", "Fixture ID"],
		["icon", "Icon"],
		["name", "Name"],
		["patch", "Patch address"],
		["intensity", "Intensity"],
		["color", "Color"],
		["position", "Position"],
		["beam", "Beam"],
		["shapers", "Shapers"],
		["focus", "Focus"],
		["control", "Control"],
		["media", "Media"],
	] as const;
	return (
		<div className="fixed-screen-pane-settings">
			<SelectField
				label="Compact mode"
				value={pane.compact_mode}
				onChange={(compact_mode) => update({ ...pane, compact_mode })}
				options={[
					{ value: "off", label: "Off" },
					{ value: "icon_only", label: "Icon only" },
					{ value: "text_only", label: "Text only" },
				]}
			/>
			<SelectField
				label="Fixture heads"
				value={pane.included_heads}
				onChange={(included_heads) => update({ ...pane, included_heads })}
				options={[
					{ value: "all", label: "All" },
					{ value: "no_sub_heads", label: "No sub heads" },
					{ value: "no_master_heads", label: "No master heads" },
				]}
			/>
			<SelectField
				label="Ordering"
				value={pane.order}
				onChange={(order) => update({ ...pane, order })}
				options={[
					{ value: "fixture_id", label: "Fixture ID" },
					{ value: "active", label: "Active fixtures first" },
				]}
			/>
			<SwitchField
				label="Fixture filter"
				offLabel="All fixtures"
				onLabel="Active only"
				checked={pane.active_only}
				onChange={(event) =>
					update({ ...pane, active_only: event.target.checked })
				}
			/>
			<SelectField
				label="Cuelist filter"
				value={pane.cue_list_id ?? ""}
				onChange={(cue_list_id) =>
					update({ ...pane, cue_list_id: cue_list_id || null })
				}
				options={[
					{ value: "", label: "All fixtures" },
					...(pane.cue_list_id &&
					!cueLists.some((cueList) => cueList.id === pane.cue_list_id)
						? [
								{
									value: pane.cue_list_id,
									label: "Configured Cuelist is unavailable",
								},
							]
						: []),
					...cueLists.map((cueList) => ({
						value: cueList.id,
						label: cueList.name,
					})),
				]}
			/>
			<SwitchField
				label="Name details"
				offLabel="Names only"
				onLabel="Show fixture type"
				checked={pane.show_type}
				onChange={(event) =>
					update({ ...pane, show_type: event.target.checked })
				}
			/>
			<SwitchField
				label="Group shortcuts"
				offLabel="Hidden"
				onLabel="Visible"
				checked={pane.show_group_shortcuts}
				onChange={(event) =>
					update({ ...pane, show_group_shortcuts: event.target.checked })
				}
			/>
			<fieldset className="fixed-screen-column-settings">
				<legend>Columns</legend>
				{columnOptions.map(([column, label]) => (
					<SwitchField
						key={column}
						label={label}
						offLabel="Hidden"
						onLabel="Visible"
						checked={pane.columns.includes(column)}
						disabled={pane.columns.length === 1 && pane.columns[0] === column}
						onChange={(event) =>
							update({
								...pane,
								columns: event.target.checked
									? [...pane.columns, column]
									: pane.columns.filter((candidate) => candidate !== column),
							})
						}
					/>
				))}
			</fieldset>
		</div>
	);
}

function Stage2dFixedSettings({
	pane,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "stage_2d" }>;
	update: (pane: FixedScreenPane) => void;
}) {
	return (
		<div className="fixed-screen-pane-settings">
			<SwitchField
				label="Preload source"
				offLabel="Live"
				onLabel="Follow preload"
				checked={pane.follow_preload}
				onChange={(event) =>
					update({ ...pane, follow_preload: event.target.checked })
				}
			/>
		</div>
	);
}

function Stage3dFixedSettings({
	pane,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "stage_3d" }>;
	update: (pane: FixedScreenPane) => void;
}) {
	return (
		<div className="fixed-screen-pane-settings">
			<SwitchField
				label="Preload source"
				offLabel="Live"
				onLabel="Follow preload"
				checked={pane.follow_preload}
				onChange={(event) =>
					update({ ...pane, follow_preload: event.target.checked })
				}
			/>
			<SwitchField
				label="Floor grid"
				offLabel="Hidden"
				onLabel="Visible"
				checked={pane.show_floor_grid}
				onChange={(event) =>
					update({ ...pane, show_floor_grid: event.target.checked })
				}
			/>
			<SwitchField
				label="Beam direction guidelines"
				offLabel="Hidden"
				onLabel="Visible"
				checked={pane.show_beam_guides}
				onChange={(event) =>
					update({ ...pane, show_beam_guides: event.target.checked })
				}
			/>
			<SelectField
				label="Render quality"
				value={pane.render_quality}
				onChange={(render_quality) => update({ ...pane, render_quality })}
				options={[
					{ value: "lines_only", label: "Lines only" },
					{ value: "lines_and_beams", label: "Lines and beams" },
					{ value: "full", label: "Full" },
				]}
			/>
			<NumberField
				label="Environment brightness"
				min="0"
				max="1"
				step="0.05"
				value={pane.environment_brightness}
				onChange={(event) =>
					update({
						...pane,
						environment_brightness: Number(event.target.value),
					})
				}
			/>
		</div>
	);
}

function CuesFixedSettings({
	pane,
	cueLists,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "cues" }>;
	cueLists: readonly CuelistOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	return (
		<SelectField
			label="Cuelist"
			value={pane.cue_list_id}
			onChange={(cue_list_id) => update({ ...pane, cue_list_id })}
			options={[
				...(!pane.cue_list_id
					? [{ value: "", label: "Unavailable - choose a Cuelist" }]
					: cueLists.some((cueList) => cueList.id === pane.cue_list_id)
						? []
						: [
								{
									value: pane.cue_list_id,
									label: "Configured Cuelist is unavailable",
								},
							]),
				...cueLists.map((cueList) => ({
					value: cueList.id,
					label: cueList.name,
				})),
			]}
		/>
	);
}

function TextFixedSettings({
	pane,
	textFiles,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "text" }>;
	textFiles: readonly TextFileOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	const selectedTextValue =
		pane.root && pane.path ? `${pane.root}\u0000${pane.path}` : "";
	return (
		<div className="fixed-screen-pane-settings">
			<SelectField
				label="Text"
				value={selectedTextValue}
				onChange={(value) => {
					const [root = "", path = ""] = value.split("\u0000");
					update({ ...pane, root, path });
				}}
				options={[
					...(!selectedTextValue
						? [{ value: "", label: "Unavailable - choose a text file" }]
						: textFiles.some(
									(file) => file.root === pane.root && file.path === pane.path,
								)
							? []
							: [
									{
										value: selectedTextValue,
										label: "Configured text is unavailable",
									},
								]),
					...textFiles.map((file) => ({
						value: `${file.root}\u0000${file.path}`,
						label: `${file.rootLabel} · ${file.name}`,
					})),
				]}
			/>
			<SelectField
				label="Text view"
				value={pane.mode}
				onChange={(mode) => update({ ...pane, mode })}
				options={[
					{ value: "plain", label: "Plain Text" },
					{ value: "markdown", label: "Rendered Markdown" },
				]}
			/>
		</div>
	);
}

function FixedPaneSettings({
	pane,
	cueLists,
	textFiles,
	update,
}: {
	pane: FixedScreenPane;
	cueLists: readonly CuelistOption[];
	textFiles: readonly TextFileOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	if (pane.type === "fixture_sheet")
		return (
			<FixtureSheetFixedSettings
				pane={pane}
				cueLists={cueLists}
				update={update}
			/>
		);
	if (pane.type === "stage_2d")
		return <Stage2dFixedSettings pane={pane} update={update} />;
	if (pane.type === "stage_3d")
		return <Stage3dFixedSettings pane={pane} update={update} />;
	if (pane.type === "cues")
		return (
			<CuesFixedSettings pane={pane} cueLists={cueLists} update={update} />
		);
	return (
		<TextFixedSettings pane={pane} textFiles={textFiles} update={update} />
	);
}

interface ScreenSettingsFieldsProps {
	draft: ScreenConfiguration;
	desks: DeskModel[];
	displays: Array<{ id: string; name: string }>;
	cueLists: readonly CuelistOption[];
	textFiles: readonly TextFileOption[];
	update: (changes: Partial<ScreenConfiguration>) => void;
}

function ScreenLayoutFields({
	draft,
	desks,
	update,
}: Pick<ScreenSettingsFieldsProps, "draft" | "desks" | "update">) {
	const fixedPane =
		draft.content.type === "desktop" ? null : draft.content.pane;
	return (
		<section>
			<h3>Layout</h3>
			<div className="screen-settings-fields">
				<SelectField
					label="Content"
					value={
						draft.content.type === "fixed_side_pane"
							? `fixed_side_pane_${draft.content.side}`
							: draft.content.type
					}
					onChange={(type) =>
						update({
							content:
								type === "fixed_pane"
									? {
											type,
											pane: fixedPane ?? DEFAULT_FIXED_SCREEN_PANE,
										}
									: type === "fixed_side_pane_left"
										? {
												type: "fixed_side_pane",
												pane: fixedPane ?? DEFAULT_FIXED_SCREEN_PANE,
												side: "left",
												width_px:
													draft.content.type === "fixed_side_pane"
														? draft.content.width_px
														: DEFAULT_FIXED_SIDE_WIDTH_PX,
											}
										: type === "fixed_side_pane_right"
											? {
													type: "fixed_side_pane",
													pane: fixedPane ?? DEFAULT_FIXED_SCREEN_PANE,
													side: "right",
													width_px:
														draft.content.type === "fixed_side_pane"
															? draft.content.width_px
															: DEFAULT_FIXED_SIDE_WIDTH_PX,
												}
											: { type: "desktop" },
						})
					}
					options={[
						{ value: "desktop", label: "Desktop" },
						{ value: "fixed_pane", label: "Fixed full-screen pane" },
						{ value: "fixed_side_pane_left", label: "Fixed left pane" },
						{ value: "fixed_side_pane_right", label: "Fixed right pane" },
					]}
				/>
				{draft.content.type !== "fixed_pane" && (
					<SelectField
						label="Desktop"
						value={draft.layout.activeDeskId}
						onChange={(activeDeskId) =>
							update({ layout: { desks, activeDeskId } })
						}
						options={desks.map((desk) => ({
							value: desk.id,
							label: desk.name,
						}))}
					/>
				)}
				<SwitchField
					label="Dock"
					offLabel="Hidden"
					onLabel="Visible"
					checked={draft.show_dock}
					disabled={draft.content.type === "fixed_pane"}
					description={
						draft.content.type === "fixed_pane"
							? "Dock is unavailable with a fixed full-screen pane."
							: undefined
					}
					onChange={(event) => update({ show_dock: event.target.checked })}
				/>
				<SwitchField
					label="Playbacks"
					offLabel="Hidden"
					onLabel="Visible"
					checked={draft.show_playbacks}
					onChange={(event) => update({ show_playbacks: event.target.checked })}
				/>
				<SwitchField
					label="Page controls"
					offLabel="Hidden"
					onLabel="Visible"
					checked={draft.show_page_controls}
					onChange={(event) =>
						update({ show_page_controls: event.target.checked })
					}
				/>
			</div>
		</section>
	);
}

function ScreenPaneSettings({
	draft,
	cueLists,
	textFiles,
	update,
}: Pick<
	ScreenSettingsFieldsProps,
	"draft" | "cueLists" | "textFiles" | "update"
>) {
	const fixedContent =
		draft.content.type === "desktop" ? null : draft.content;
	const fixedPane = fixedContent?.pane ?? null;
	const sideContent =
		draft.content.type === "fixed_side_pane" ? draft.content : null;
	return (
		<section>
			<h3>Settings</h3>
			<div className="screen-settings-fields">
				{fixedPane ? (
					<>
						<SelectField
							label="Pane"
							value={fixedPane.type}
							onChange={(type) => {
								if (!fixedContent) return;
								update({
									content: {
										...fixedContent,
										pane: defaultFixedPane(type),
									},
								});
							}}
							options={Object.entries(fixedPaneLabels).map(
								([value, label]) => ({
									value: value as FixedScreenPane["type"],
									label,
								}),
							)}
						/>
						<FixedPaneSettings
							pane={fixedPane}
							cueLists={cueLists}
							textFiles={textFiles}
							update={(pane) => {
								if (fixedContent)
									update({ content: { ...fixedContent, pane } });
							}}
						/>
						{sideContent ? (
							<NumberField
								label="Pane width (px)"
								min={240}
								max={1200}
								value={sideContent.width_px}
								onChange={(event) =>
									update({
										content: {
											...sideContent,
											width_px: Number(event.target.value),
										},
									})
								}
							/>
						) : null}
					</>
				) : (
					<p className="screen-settings-note">
						This screen follows the selected Desktop layout.
					</p>
				)}
			</div>
		</section>
	);
}

function ScreenPlacementFields({
	draft,
	displays,
	update,
}: Pick<ScreenSettingsFieldsProps, "draft" | "displays" | "update">) {
	const bounds = (
		changes: Partial<NonNullable<ScreenConfiguration["bounds"]>>,
	) => ({
		x: draft.bounds?.x ?? 0,
		y: draft.bounds?.y ?? 0,
		width: draft.bounds?.width ?? 1280,
		height: draft.bounds?.height ?? 720,
		...changes,
	});
	return (
		<section>
			<h3>Placement</h3>
			<div className="screen-settings-fields">
				<SelectField
					label="Physical Display"
					value={draft.display_id ?? ""}
					onChange={(value) => update({ display_id: value || null })}
					options={[
						{ value: "", label: "Choose when opened" },
						...displays.map((display) => ({
							value: display.id,
							label: display.name,
						})),
					]}
				/>
				<SwitchField
					label="Window mode"
					offLabel="Windowed"
					onLabel="Fullscreen"
					checked={draft.fullscreen}
					onChange={(event) => update({ fullscreen: event.target.checked })}
				/>
				<FormLayout columns={2} minColumnWidth={90}>
					<NumberField
						label="Window X"
						value={draft.bounds?.x ?? 0}
						onChange={(event) =>
							update({ bounds: bounds({ x: Number(event.target.value) }) })
						}
					/>
					<NumberField
						label="Window Y"
						value={draft.bounds?.y ?? 0}
						onChange={(event) =>
							update({ bounds: bounds({ y: Number(event.target.value) }) })
						}
					/>
					<NumberField
						label="Window width"
						min="1"
						value={draft.bounds?.width ?? 1280}
						onChange={(event) =>
							update({
								bounds: bounds({ width: Number(event.target.value) }),
							})
						}
					/>
					<NumberField
						label="Window height"
						min="1"
						value={draft.bounds?.height ?? 720}
						onChange={(event) =>
							update({
								bounds: bounds({ height: Number(event.target.value) }),
							})
						}
					/>
				</FormLayout>
			</div>
		</section>
	);
}

function ScreenSettingsFields(props: ScreenSettingsFieldsProps) {
	return (
		<div className="screen-settings-columns">
			<ScreenLayoutFields
				draft={props.draft}
				desks={props.desks}
				update={props.update}
			/>
			<ScreenPaneSettings
				draft={props.draft}
				cueLists={props.cueLists}
				textFiles={props.textFiles}
				update={props.update}
			/>
			<ScreenPlacementFields
				draft={props.draft}
				displays={props.displays}
				update={props.update}
			/>
		</div>
	);
}

export function ScreenSettingsCard({
	screen,
	desks = screen.layout.desks,
	displays,
	cueLists = [],
	textFiles = [],
	save,
	remove,
}: {
	screen: ScreenConfiguration;
	desks?: DeskModel[];
	displays: Array<{ id: string; name: string }>;
	cueLists?: readonly CuelistOption[];
	textFiles?: readonly TextFileOption[];
	save: (screen: ScreenConfiguration) => Promise<void>;
	remove: (screen: ScreenConfiguration) => Promise<void>;
}) {
	const [draft, setDraft] = useState(screen);
	const [playbackModalOpen, setPlaybackModalOpen] = useState(false);
	const draftRef = useRef(screen);
	const saveQueue = useRef(Promise.resolve());
	const pending = useRef(0);
	useEffect(() => {
		if (pending.current === 0) {
			draftRef.current = screen;
			setDraft(screen);
		}
	}, [screen]);
	const update = (changes: Partial<ScreenConfiguration>) => {
		const next = updateScreenConfiguration(draftRef.current, changes);
		draftRef.current = next;
		setDraft(next);
		pending.current += 1;
		saveQueue.current = saveQueue.current
			.then(() => save(next))
			.finally(() => {
				pending.current -= 1;
			});
	};
	return (
		<article
			className="screen-settings-card"
			aria-label={`Screen ${draft.name}`}
			data-screen-id={draft.id}
		>
			<header className="screen-settings-header">
				<TextField
					aria-label="Screen name"
					value={draft.name}
					onChange={(event) => update({ name: event.target.value })}
				/>
				<div className="screen-settings-actions">
					<a
						className="ui-button ui-secondary ui-default"
						href={browserScreenUrl(draft.id, window.location.href)}
						target="_blank"
						rel="noreferrer"
					>
						Open browser view
					</a>
					<Button onClick={() => setPlaybackModalOpen(true)}>
						Configure Playbacks
					</Button>
					<Button
						variant={draft.desired_open ? "warning" : "success"}
						onClick={() => update({ desired_open: !draft.desired_open })}
					>
						{draft.desired_open ? "Close Screen" : "Open Screen"}
					</Button>
					<Button variant="danger" onClick={() => void remove(draft)}>
						Remove Screen
					</Button>
				</div>
			</header>
			<ScreenSettingsFields
				draft={draft}
				desks={desks}
				displays={displays}
				cueLists={cueLists}
				textFiles={textFiles}
				update={update}
			/>
			{playbackModalOpen && (
				<PlaybackLayoutModal
					initialLayout={screenPlaybackLayout(draft)}
					pageMode={draft.page_mode}
					onClose={() => setPlaybackModalOpen(false)}
					onSave={(playback_layout, page_mode) => {
						const legacy = playbackLayoutLegacyFields(playback_layout);
						update({
							playback_layout,
							page_mode,
							playback_count: legacy.playback_count,
							playback_rows: legacy.playback_rows,
							first_playback_slot: legacy.first_playback_slot,
						});
						setPlaybackModalOpen(false);
					}}
				/>
			)}
		</article>
	);
}

export function DefaultScreenSettings({
	deskName,
	deskAlias,
	playbackLayout,
	fallbackColumns,
	fallbackRows,
	playbackSlots,
	keyboardShortcuts,
	onName,
	onAlias,
	onTextFocus,
	onTextBlur,
	onKeyboardShortcuts,
	onConfigurePlaybacks,
	onChooseDefault,
}: {
	deskName: string;
	deskAlias: string;
	playbackLayout: PlaybackSurfaceLayout | null;
	fallbackColumns: number;
	fallbackRows: number;
	playbackSlots: number;
	keyboardShortcuts: boolean;
	onName: (name: string) => void;
	onAlias: (alias: string) => void;
	onTextFocus: (field: "name" | "osc_alias") => void;
	onTextBlur: (field: "name" | "osc_alias") => void;
	onKeyboardShortcuts: (enabled: boolean) => void;
	onConfigurePlaybacks: () => void;
	onChooseDefault: () => void;
}) {
	return (
		<article className="default-screen-settings">
			<header>
				<div>
					<b>Default screen</b>
					<small>Primary desk window</small>
				</div>
			</header>
			<FormLayout
				className="screen-settings-grid"
				columns={3}
				minColumnWidth={180}
			>
				<TextField
					label="Name"
					value={deskName}
					onFocus={() => onTextFocus("name")}
					onBlur={() => onTextBlur("name")}
					onChange={(event) => onName(event.target.value)}
				/>
				<TextField
					label="OSC alias"
					value={deskAlias}
					onFocus={() => onTextFocus("osc_alias")}
					onBlur={() => onTextBlur("osc_alias")}
					onChange={(event) => onAlias(event.target.value)}
				/>
				<div className="playback-layout-summary">
					<b>Playback surface</b>
					<small>
						{playbackLayout?.rows.length ?? fallbackRows} rows ·{" "}
						{playbackLayout?.playbacks_per_row ?? fallbackColumns} playbacks per
						row
					</small>
				</div>
				<SwitchField
					label="Enable software keyboard shortcuts"
					offLabel="Disabled"
					onLabel="Enabled"
					checked={keyboardShortcuts}
					description="Keyboard shortcuts are always disabled while hardware controls are connected."
					onChange={(event) => onKeyboardShortcuts(event.target.checked)}
				/>
			</FormLayout>
			<footer className="default-screen-status">
				<small>
					{playbackSlots} playback slots · OSC /light/{deskAlias || "desk"}/
				</small>
				<div className="screen-settings-actions default-screen-bottom-actions">
					<Button onClick={onConfigurePlaybacks}>Configure Playbacks</Button>
					<Button onClick={onChooseDefault}>Choose default screen</Button>
				</div>
			</footer>
		</article>
	);
}
