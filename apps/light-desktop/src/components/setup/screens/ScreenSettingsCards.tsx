import { useEffect, useRef, useState } from "react";
import type {
	PlaybackSurfaceLayout,
	ScreenConfiguration,
} from "../../../api/types";
import type { DeskModel } from "../../../types";
import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "../../common";
import { PlaybackLayoutModal } from "../PlaybackLayoutModal";
import {
	playbackLayoutLegacyFields,
	screenPlaybackLayout,
} from "../screenConfiguration";

function ScreenSettingsFields({
	draft,
	desks,
	displays,
	update,
}: {
	draft: ScreenConfiguration;
	desks: DeskModel[];
	displays: Array<{ id: string; name: string }>;
	update: (changes: Partial<ScreenConfiguration>) => void;
}) {
	const bounds = (changes: Partial<NonNullable<ScreenConfiguration["bounds"]>>) => ({
		x: draft.bounds?.x ?? 0,
		y: draft.bounds?.y ?? 0,
		width: draft.bounds?.width ?? 1280,
		height: draft.bounds?.height ?? 720,
		...changes,
	});
	return <div className="screen-settings-columns">
		<section><h3>Layout</h3><div className="screen-settings-fields">
			<SelectField label="Desktop" value={draft.layout.activeDeskId} onChange={(activeDeskId) => update({ layout: { desks, activeDeskId } })} options={desks.map((desk) => ({ value: desk.id, label: desk.name }))}/>
			<SwitchField label="Show Dock" checked={draft.show_dock} onChange={(event) => update({ show_dock: event.target.checked })}/>
			<SwitchField label="Show Playbacks" checked={draft.show_playbacks} onChange={(event) => update({ show_playbacks: event.target.checked })}/>
			<SwitchField label="Show Page Controls" checked={draft.show_page_controls} onChange={(event) => update({ show_page_controls: event.target.checked })}/>
		</div></section>
		<section><h3>Placement</h3><div className="screen-settings-fields">
			<SelectField label="Physical Display" value={draft.display_id ?? ""} onChange={(value) => update({ display_id: value || null })} options={[{ value: "", label: "Choose when opened" }, ...displays.map((display) => ({ value: display.id, label: display.name }))]}/>
			<SwitchField label="Fullscreen" checked={draft.fullscreen} onChange={(event) => update({ fullscreen: event.target.checked })}/>
			<FormLayout columns={2} minColumnWidth={90}>
				<NumberField label="Window X" value={draft.bounds?.x ?? 0} onChange={(event) => update({ bounds: bounds({ x: Number(event.target.value) }) })}/>
				<NumberField label="Window Y" value={draft.bounds?.y ?? 0} onChange={(event) => update({ bounds: bounds({ y: Number(event.target.value) }) })}/>
				<NumberField label="Window width" min="1" value={draft.bounds?.width ?? 1280} onChange={(event) => update({ bounds: bounds({ width: Number(event.target.value) }) })}/>
				<NumberField label="Window height" min="1" value={draft.bounds?.height ?? 720} onChange={(event) => update({ bounds: bounds({ height: Number(event.target.value) }) })}/>
			</FormLayout>
		</div></section>
		<section><h3>Playbacks</h3><div className="screen-settings-fields"><p className="playback-layout-summary">
			{screenPlaybackLayout(draft).rows.length} rows · {screenPlaybackLayout(draft).playbacks_per_row} playbacks per row · {draft.page_mode === "follow_main" ? "Follow Main" : "Dedicated Page"}
		</p></div></section>
	</div>;
}

export function ScreenSettingsCard({
	screen,
	desks = screen.layout.desks,
	displays,
	save,
	remove,
}: {
	screen: ScreenConfiguration;
	desks?: DeskModel[];
	displays: Array<{ id: string; name: string }>;
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
		const next = { ...draftRef.current, ...changes };
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
		<article className="screen-settings-card" aria-label={`Screen ${draft.name}`} data-screen-id={draft.id}>
			<header className="screen-settings-header">
				<TextField
					aria-label="Screen name"
					value={draft.name}
					onChange={(event) => update({ name: event.target.value })}
				/>
				<div className="screen-settings-actions">
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
			<ScreenSettingsFields draft={draft} desks={desks} displays={displays} update={update}/>
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
