import {
	Button,
	FormLayout,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import { configuredServerUrl } from "../../../api/client/serverLocation";
import type {
	FixedScreenPane,
	ProgrammerControlSurfacePatch,
	ScreenConfiguration,
} from "../../../api/types";
import type { DeskModel } from "../../../types";
import { PlaybackLayoutModal } from "../PlaybackLayoutModal";
import {
	browserScreenUrl,
	DEFAULT_FIXED_SCREEN_PANE,
	DEFAULT_FIXED_SIDE_WIDTH_PERCENT,
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

/** Content that dedicates its whole layout to the control region. */
function hasControlSurface(content: ScreenConfiguration["content"]) {
	return (
		content.type === "control_surface" || content.type === "fixed_side_pane"
	);
}

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
	const fixedContent =
		draft.content.type === "fixed_pane" ||
		draft.content.type === "fixed_side_pane"
			? draft.content
			: null;
	const fixedPane = fixedContent?.pane ?? null;
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
									: type === "control_surface"
										? { type }
										: type === "fixed_side_pane_left" ||
												type === "fixed_side_pane_right"
											? {
													type: "fixed_side_pane",
													pane: fixedPane ?? DEFAULT_FIXED_SCREEN_PANE,
													side:
														type === "fixed_side_pane_left" ? "left" : "right",
													width_percent:
														draft.content.type === "fixed_side_pane"
															? draft.content.width_percent
															: DEFAULT_FIXED_SIDE_WIDTH_PERCENT,
												}
											: { type: "desktop" },
						})
					}
					options={[
						{ value: "desktop", label: "Desktop" },
						{ value: "control_surface", label: "Controls only" },
						{ value: "fixed_pane", label: "Fixed full-screen pane" },
						{ value: "fixed_side_pane_left", label: "Fixed left pane" },
						{ value: "fixed_side_pane_right", label: "Fixed right pane" },
					]}
				/>
				{fixedPane && (
					<SelectField
						label="Pane"
						value={fixedPane.type}
						onChange={(type) => {
							if (!fixedContent) return;
							update({
								content: { ...fixedContent, pane: defaultFixedPane(type) },
							});
						}}
						options={Object.entries(fixedPaneLabels).map(([value, label]) => ({
							value: value as FixedScreenPane["type"],
							label,
						}))}
					/>
				)}
				{draft.content.type === "desktop" && (
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
					disabled={draft.content.type !== "desktop"}
					description={
						draft.content.type === "fixed_pane"
							? "Dock is unavailable with a fixed full-screen pane."
							: draft.content.type === "fixed_side_pane"
								? "Dock is unavailable with a fixed side pane."
								: draft.content.type === "control_surface"
									? "Dock is unavailable without Desktop content."
									: undefined
					}
					onChange={(event) => update({ show_dock: event.target.checked })}
				/>
				<SwitchField
					label="Playbacks"
					offLabel="Hidden"
					onLabel="Visible"
					checked={draft.show_playbacks}
					onChange={(event) =>
						update(
							event.target.checked
								? { show_playbacks: true }
								: /* Without Playbacks there is no page to control. */
									{ show_playbacks: false, show_page_controls: false },
						)
					}
				/>
				{draft.show_playbacks && (
					<SwitchField
						label="Page controls"
						offLabel="Hidden"
						onLabel="Visible"
						checked={draft.show_page_controls}
						onChange={(event) =>
							update({ show_page_controls: event.target.checked })
						}
					/>
				)}
				<SwitchField
					label="Command line"
					offLabel="Hidden"
					onLabel="Visible"
					checked={draft.show_programmer}
					onChange={(event) =>
						update({ show_programmer: event.target.checked })
					}
					description="The encoder section of this screen always keeps the keypad, the programmer fader and the Delete/Move tools on the main screen. Visible adds the command line above its encoders."
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
		draft.content.type === "fixed_pane" ||
		draft.content.type === "fixed_side_pane"
			? draft.content
			: null;
	const fixedPane = fixedContent?.pane ?? null;
	const sideContent =
		draft.content.type === "fixed_side_pane" ? draft.content : null;
	return (
		<section>
			<h3>Settings</h3>
			<div className="screen-settings-fields">
				{fixedPane ? (
					<>
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
								label="Pane width (%)"
								min={10}
								max={80}
								value={sideContent.width_percent}
								onChange={(event) =>
									update({
										content: {
											...sideContent,
											width_percent: Number(event.target.value),
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
				{/* Stacked: the four labels do not fit beside each other at this width. */}
				<FormLayout columns={1} minColumnWidth={90}>
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

function ScreenCardHeader(props: {
	draft: ScreenConfiguration;
	programmerOwner: boolean;
	update: (changes: Partial<ScreenConfiguration>) => void;
	copyBrowserLink: () => Promise<void>;
	copyState: "idle" | "success" | "error";
	openConfiguration: () => void;
	remove: () => void;
}) {
	return (
		<header className="screen-settings-header">
			<TextField
				aria-label="Screen name"
				value={props.draft.name}
				onChange={(event) => props.update({ name: event.target.value })}
			/>
			<div className="screen-settings-actions">
				<Button
					variant={
						props.copyState === "success"
							? "success"
							: props.copyState === "error"
								? "warning"
								: "secondary"
					}
					onClick={() => void props.copyBrowserLink()}
				>
					{props.copyState === "success"
						? "✓ Copied browser link"
						: props.copyState === "error"
							? "Copy failed"
							: "Copy browser link"}
				</Button>
				<Button onClick={props.openConfiguration}>Configure screen</Button>
				<Button
					variant={props.draft.desired_open ? "warning" : "success"}
					onClick={() =>
						props.update({ desired_open: !props.draft.desired_open })
					}
				>
					{props.draft.desired_open ? "Close Screen" : "Open Screen"}
				</Button>
				<Button variant="danger" onClick={props.remove}>
					Remove Screen
				</Button>
			</div>
		</header>
	);
}

type ScreenConfigurationTab = "layout" | "settings" | "placement" | "playbacks";

function ScreenConfigurationModal(
	props: ScreenSettingsFieldsProps & {
		programmerOwner: boolean;
		onClose: () => void;
		onConfigurePlaybacks: () => void;
	},
) {
	const [tab, setTab] = useState<ScreenConfigurationTab>("layout");
	return (
		<ModalRegistration onClose={props.onClose}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal screen-configuration-modal"
					role="dialog"
					aria-modal="true"
					aria-label={`Configure ${props.draft.name}`}
				>
					<ModalTitleBar
						title={`Configure ${props.draft.name}`}
						groups={[
							{
								id: "screen-configuration-tabs",
								kind: "tabs",
								activeId: tab,
								onActiveChange: (id) => setTab(id as ScreenConfigurationTab),
								actions: [
									{ id: "layout", label: "Layout" },
									{ id: "settings", label: "Settings" },
									{ id: "placement", label: "Placement" },
									{ id: "playbacks", label: "Playbacks" },
								],
							},
						]}
						closeLabel="Close screen configuration"
						onClose={props.onClose}
					/>
					<div className="screen-configuration-modal-content">
						{tab === "layout" && (
							<ScreenLayoutFields
								draft={props.draft}
								desks={props.desks}
								update={props.update}
							/>
						)}
						{tab === "settings" && (
							<>
								<ScreenControlSurfaceNote
									draft={props.draft}
									programmerOwner={props.programmerOwner}
								/>
								<ScreenPaneSettings
									draft={props.draft}
									cueLists={props.cueLists}
									textFiles={props.textFiles}
									update={props.update}
								/>
							</>
						)}
						{tab === "placement" && (
							<ScreenPlacementFields
								draft={props.draft}
								displays={props.displays}
								update={props.update}
							/>
						)}
						{tab === "playbacks" && (
							<div className="screen-playbacks-tab">
								<p>
									Configure the Playback rows, faders, buttons, and page mode.
								</p>
								<Button variant="primary" onClick={props.onConfigurePlaybacks}>
									Configure Playbacks
								</Button>
							</div>
						)}
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}

function ScreenControlSurfaceNote({
	draft,
	programmerOwner,
}: {
	draft: ScreenConfiguration;
	programmerOwner: boolean;
}) {
	const carries = hasControlSurface(draft.content);
	if (carries && !programmerOwner)
		return (
			<p className="screen-settings-note" role="status">
				This control layout becomes active when this screen carries the
				encoders. Selecting it assigns that placement when saved.
			</p>
		);
	if (programmerOwner && !carries)
		return (
			<p className="screen-settings-note" role="status">
				The encoders appear below this content. When this screen also shows
				Playbacks, the Playback/Encoders switch sits beside the section's own
				controls.
			</p>
		);
	if (programmerOwner)
		return (
			<p className="screen-settings-note" role="status">
				This control layout carries the encoders over the full screen height.
				When Playbacks are also enabled, the Playback/Encoders switch sits
				beside the section's own controls.
			</p>
		);
	return null;
}

function ScreenRemovalConfirmation(props: {
	draft: ScreenConfiguration;
	removing: boolean;
	canUpdateOwner: boolean;
	confirm: () => Promise<void>;
	cancel: () => void;
	error: string | null;
}) {
	return (
		<div
			className="screen-owner-remove-confirmation"
			role="dialog"
			aria-label={`Remove ${props.draft.name}`}
		>
			<ModalTitleBar
				title={`Remove ${props.draft.name}`}
				onClose={props.cancel}
			/>
			<b>{props.draft.name} carries the encoders.</b>
			<p>
				Removing it will move the encoders back to the main screen in the same
				confirmed action.
			</p>
			<div>
				<Button
					variant="danger"
					disabled={props.removing || !props.canUpdateOwner}
					onClick={() => void props.confirm()}
				>
					{props.removing
						? "Removing…"
						: "Remove and use encoders on main screen"}
				</Button>
				<Button disabled={props.removing} onClick={props.cancel}>
					Cancel
				</Button>
			</div>
			{props.error && <p role="alert">{props.error}</p>}
		</div>
	);
}

async function copyScreenBrowserLink(browserLink: string) {
	try {
		if (!navigator.clipboard)
			throw new Error("Clipboard access is unavailable in this browser.");
		await navigator.clipboard.writeText(browserLink);
		return true;
	} catch (error) {
		void error;
		return false;
	}
}

export function ScreenSettingsCard({
	screen,
	desks = screen.layout.desks,
	displays,
	cueLists = [],
	textFiles = [],
	save,
	remove,
	programmerOwner = false,
	updateProgrammerOwner,
}: {
	screen: ScreenConfiguration;
	desks?: DeskModel[];
	displays: Array<{ id: string; name: string }>;
	cueLists?: readonly CuelistOption[];
	textFiles?: readonly TextFileOption[];
	save: (screen: ScreenConfiguration) => Promise<void>;
	remove: (screen: ScreenConfiguration) => Promise<void>;
	programmerOwner?: boolean;
	updateProgrammerOwner?: (
		patch: ProgrammerControlSurfacePatch,
	) => Promise<void>;
}) {
	const [draft, setDraft] = useState(screen);
	const [playbackModalOpen, setPlaybackModalOpen] = useState(false);
	const [configurationOpen, setConfigurationOpen] = useState(false);
	const [copyState, setCopyState] = useState<"idle" | "success" | "error">(
		"idle",
	);
	const [removeConfirmationOpen, setRemoveConfirmationOpen] = useState(false);
	const [removeError, setRemoveError] = useState<string | null>(null);
	const [removing, setRemoving] = useState(false);
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
		const previous = draftRef.current;
		const next = updateScreenConfiguration(previous, changes);
		draftRef.current = next;
		setDraft(next);
		pending.current += 1;
		saveQueue.current = saveQueue.current
			.then(async () => {
				await save(next);
				if (
					hasControlSurface(next.content) &&
					!hasControlSurface(previous.content)
				)
					await updateProgrammerOwner?.({ owner_screen_id: next.id });
				else if (
					programmerOwner &&
					hasControlSurface(previous.content) &&
					!hasControlSurface(next.content)
				)
					await updateProgrammerOwner?.({ assign_to_main: true });
			})
			.finally(() => {
				pending.current -= 1;
			});
	};
	const browserLink = browserScreenUrl(draft.id, configuredServerUrl());
	const copyBrowserLink = async () => {
		setCopyState(
			(await copyScreenBrowserLink(browserLink)) ? "success" : "error",
		);
	};
	useEffect(() => {
		if (copyState === "idle") return;
		const timer = window.setTimeout(() => setCopyState("idle"), 2500);
		return () => window.clearTimeout(timer);
	}, [copyState]);
	const confirmOwnerRemoval = async () => {
		if (!updateProgrammerOwner || removing) return;
		setRemoving(true);
		setRemoveError(null);
		try {
			await updateProgrammerOwner({ assign_to_main: true });
			await remove(draftRef.current);
		} catch (error) {
			setRemoveError(
				error instanceof Error
					? error.message
					: "Could not remove this screen.",
			);
			setRemoving(false);
		}
	};
	return (
		<article
			className="screen-settings-card"
			aria-label={`Screen ${draft.name}`}
			data-screen-id={draft.id}
		>
			<ScreenCardHeader
				draft={draft}
				programmerOwner={programmerOwner}
				update={update}
				copyBrowserLink={copyBrowserLink}
				copyState={copyState}
				openConfiguration={() => setConfigurationOpen(true)}
				remove={() =>
					programmerOwner ? setRemoveConfirmationOpen(true) : void remove(draft)
				}
			/>
			{removeConfirmationOpen && (
				<ScreenRemovalConfirmation
					draft={draft}
					removing={removing}
					canUpdateOwner={Boolean(updateProgrammerOwner)}
					confirm={confirmOwnerRemoval}
					cancel={() => setRemoveConfirmationOpen(false)}
					error={removeError}
				/>
			)}
			{configurationOpen && (
				<ScreenConfigurationModal
					draft={draft}
					desks={desks}
					displays={displays}
					cueLists={cueLists}
					textFiles={textFiles}
					update={update}
					programmerOwner={programmerOwner}
					onClose={() => setConfigurationOpen(false)}
					onConfigurePlaybacks={() => setPlaybackModalOpen(true)}
				/>
			)}
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
	deskAlias,
	keyboardShortcuts,
	onAlias,
	onTextFocus,
	onTextBlur,
	onKeyboardShortcuts,
	onConfigurePlaybacks,
	onChooseDefault,
	singleClientMode,
	onSingleClientMode,
}: {
	deskAlias: string;
	keyboardShortcuts: boolean;
	onAlias: (alias: string) => void;
	onTextFocus: (field: "name" | "osc_alias") => void;
	onTextBlur: (field: "name" | "osc_alias") => void;
	onKeyboardShortcuts: (enabled: boolean) => void;
	onConfigurePlaybacks: () => void;
	onChooseDefault: () => void;
	singleClientMode: boolean;
	onSingleClientMode: (enabled: boolean) => void;
}) {
	return (
		<article className="default-screen-settings">
			<header>
				<div>
					<b>Default screen</b>
					<small>Primary desk window</small>
				</div>
			</header>
			<div className="default-screen-compact-row">
				<TextField
					label="OSC alias"
					value={deskAlias}
					onFocus={() => onTextFocus("osc_alias")}
					onBlur={() => onTextBlur("osc_alias")}
					onChange={(event) => onAlias(event.target.value)}
				/>
				<SwitchField
					label="Enable software keyboard shortcuts"
					offLabel="Disabled"
					onLabel="Enabled"
					checked={keyboardShortcuts}
					description="Keyboard shortcuts are always disabled while hardware controls are connected."
					onChange={(event) => onKeyboardShortcuts(event.target.checked)}
				/>
				<SwitchField
					label="Single-client mode"
					offLabel="Keep clients"
					onLabel="Clean disconnected clients"
					checked={singleClientMode}
					onChange={(event) => onSingleClientMode(event.target.checked)}
				/>
				<div className="screen-settings-actions default-screen-bottom-actions">
					<Button onClick={onConfigurePlaybacks}>Configure Playbacks</Button>
					<Button onClick={onChooseDefault}>Choose default screen</Button>
				</div>
			</div>
		</article>
	);
}
