import {
	Button,
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
	PlaybackSurfaceLayout,
	ProgrammerControlSurfacePatch,
	ScreenConfiguration,
} from "../../../api/types";
import type { DeskModel } from "../../../types";
import {
	addPlaybackRow,
	canAddPlaybackRow,
	PlaybackLayoutFields,
	type PlaybackPageMode,
	playbackLayoutInvalid,
} from "../PlaybackLayoutModal";
import {
	browserScreenUrl,
	DEFAULT_FIXED_SCREEN_PANE,
	DEFAULT_FIXED_SIDE_WIDTH_PERCENT,
	playbackLayoutLegacyFields,
	screenPlaybackLayout,
	updateScreenConfiguration,
} from "../screenConfiguration";
import {
	type CuelistOption,
	defaultFixedPane,
	FixedPaneSettings,
	fixedPaneLabels,
	type TextFileOption,
} from "./FixedPaneSettings";

/** Content that dedicates its whole layout to the control region. */
function hasControlSurface(content: ScreenConfiguration["content"]) {
	return (
		content.type === "control_surface" || content.type === "fixed_side_pane"
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
		<>
			<SelectField
				className="screen-configuration-wide"
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
					className="screen-configuration-wide"
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
					className="screen-configuration-wide"
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
			<SwitchField
				label="Command line"
				offLabel="Hidden"
				onLabel="Visible"
				checked={draft.show_programmer}
				onChange={(event) => update({ show_programmer: event.target.checked })}
				description="The encoder section of this screen always keeps the keypad, the programmer fader and the Delete/Move tools on the main screen. Visible adds the command line above its encoders."
			/>
			<SwitchField
				label="Programming"
				offLabel="Not editable"
				onLabel="Allowed"
				checked={!draft.not_editable}
				onChange={(event) => update({ not_editable: !event.target.checked })}
				description="Not editable makes this a guest screen: it still shows the fixture sheet, the Stage and the desk's values, and still runs playbacks, macros and timecodes, but it cannot record, update or assign. Use it for a repeater somebody else is standing at while you program."
			/>
			{draft.show_playbacks && (
				<SwitchField
					className="screen-configuration-wide"
					label="Page controls"
					offLabel="Hidden"
					onLabel="Visible"
					checked={draft.show_page_controls}
					onChange={(event) =>
						update({ show_page_controls: event.target.checked })
					}
				/>
			)}
		</>
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
	return fixedPane ? (
		<>
			<FixedPaneSettings
				pane={fixedPane}
				cueLists={cueLists}
				textFiles={textFiles}
				update={(pane) => {
					if (fixedContent) update({ content: { ...fixedContent, pane } });
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
		<>
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
		</>
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

/**
 * The Playbacks tab keeps its own draft so a value that cannot be stored yet (for example a
 * cleared number) stays visible across tab switches. Every storable draft applies at once, like
 * the other tabs of Configure Screen.
 */
/** A screen's playbacks must fit slots 1-127 from its first row onwards. */
function screenPlaybackDraftInvalid(layout: PlaybackSurfaceLayout) {
	const first = layout.rows[0]?.first_playback_slot ?? 1;
	return (
		playbackLayoutInvalid(layout) ||
		first + layout.playbacks_per_row * layout.rows.length - 1 > 127
	);
}

function useScreenPlaybackDraft(
	draft: ScreenConfiguration,
	update: (changes: Partial<ScreenConfiguration>) => void,
) {
	const [layout, setLayout] = useState(() =>
		structuredClone(screenPlaybackLayout(draft)),
	);
	const layoutRef = useRef(layout);
	const apply = (next: PlaybackSurfaceLayout, pageMode: PlaybackPageMode) => {
		if (screenPlaybackDraftInvalid(next)) return;
		const legacy = playbackLayoutLegacyFields(next);
		update({
			playback_layout: next,
			page_mode: pageMode,
			playback_count: legacy.playback_count,
			playback_rows: legacy.playback_rows,
			first_playback_slot: legacy.first_playback_slot,
		});
	};
	const changeLayout = (
		change: (current: PlaybackSurfaceLayout) => PlaybackSurfaceLayout,
	) => {
		const next = change(layoutRef.current);
		if (next === layoutRef.current) return;
		layoutRef.current = next;
		setLayout(next);
		apply(next, draft.page_mode);
	};
	return {
		layout,
		invalid: screenPlaybackDraftInvalid(layout),
		changeLayout,
		changePageMode: (pageMode: PlaybackPageMode) =>
			apply(layoutRef.current, pageMode),
	};
}

function ScreenConfigurationModal(
	props: ScreenSettingsFieldsProps & {
		programmerOwner: boolean;
		saveError: string | null;
		onClose: () => void;
	},
) {
	const [tab, setTab] = useState<ScreenConfigurationTab>("layout");
	const playbacks = useScreenPlaybackDraft(props.draft, props.update);
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
							...(tab === "playbacks"
								? [
										{
											id: "playback-rows",
											actions: [
												{
													id: "add-row",
													label: "Add Row",
													disabled: !canAddPlaybackRow(playbacks.layout),
													onPress: () => playbacks.changeLayout(addPlaybackRow),
												},
											],
										},
									]
								: []),
						]}
						closeLabel="Close screen configuration"
						onClose={props.onClose}
					/>
					<div className="screen-configuration-modal-content" data-tab={tab}>
						{props.saveError && (
							<p
								className="screen-settings-note screen-configuration-wide screen-save-error"
								role="alert"
							>
								Could not save this screen: {props.saveError}
							</p>
						)}
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
							<>
								<PlaybackLayoutFields
									layout={playbacks.layout}
									onLayout={playbacks.changeLayout}
									pageMode={props.draft.page_mode}
									onPageMode={playbacks.changePageMode}
								/>
								{playbacks.invalid && (
									<p className="screen-settings-note" role="alert">
										Not saved yet: use at most 32 playbacks per row and keep
										every row within playbacks 1-127, counted from the first
										row's first playback number.
									</p>
								)}
							</>
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
	const [configurationOpen, setConfigurationOpen] = useState(false);
	const [copyState, setCopyState] = useState<"idle" | "success" | "error">(
		"idle",
	);
	const [removeConfirmationOpen, setRemoveConfirmationOpen] = useState(false);
	const [removeError, setRemoveError] = useState<string | null>(null);
	const [removing, setRemoving] = useState(false);
	const draftRef = useRef(screen);
	const [saveError, setSaveError] = useState<string | null>(null);
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
			// A rejected save is reported and must not stop the saves queued after it.
			.then(
				() => setSaveError(null),
				(error: unknown) =>
					setSaveError(
						error instanceof Error ? error.message : "The desk rejected it.",
					),
			)
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
			{saveError && !configurationOpen && (
				<p className="screen-settings-note screen-save-error" role="alert">
					Could not save this screen: {saveError}
				</p>
			)}
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
					saveError={saveError}
					onClose={() => setConfigurationOpen(false)}
				/>
			)}
		</article>
	);
}

export function DefaultScreenSettings({
	deskName,
	onDeskName,
	onTextFocus,
	onTextBlur,
	keyboardShortcuts,
	onKeyboardShortcuts,
	onConfigurePlaybacks,
	onChooseDefault,
	singleClientMode,
	onSingleClientMode,
}: {
	deskName: string;
	onDeskName: (name: string) => void;
	onTextFocus: (field: "name") => void;
	onTextBlur: (field: "name") => void;
	keyboardShortcuts: boolean;
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
					label="Desk name"
					value={deskName}
					onFocus={() => onTextFocus("name")}
					onBlur={() => onTextBlur("name")}
					onChange={(event) => onDeskName(event.target.value)}
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
					<Button onClick={onChooseDefault}>Known windows</Button>
				</div>
			</div>
		</article>
	);
}
