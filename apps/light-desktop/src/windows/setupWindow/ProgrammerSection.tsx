import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import type { ReactNode } from "react";
import type {
	HighlightLookColor,
	HighlightLookConfiguration,
} from "../../api/types";
import {
	RecordDefaultsFields,
	UpdateDefaultsFields,
} from "../../components/setup/ProgrammerDefaults";
import { PoolPaletteSettings } from "../../components/shared/PoolColorSettings";
import { AttributeRegistrySettings } from "./AttributeRegistrySettings";
import type { SetupWindowController } from "./controller";

const DEFAULT_HIGHLIGHT_LOOK: HighlightLookConfiguration = {
	intensity: 1,
	color: "white",
	iris: null,
	zoom: null,
	focus: null,
	frost: null,
	compatibility: "semantic",
};

const COLOR_OPTIONS: Array<{
	value: HighlightLookColor | "ignore";
	label: string;
}> = [
	{ value: "ignore", label: "Ignore" },
	{ value: "white", label: "White" },
	{ value: "red", label: "Red" },
	{ value: "green", label: "Green" },
	{ value: "blue", label: "Blue" },
	{ value: "cyan", label: "Cyan" },
	{ value: "magenta", label: "Magenta" },
	{ value: "amber", label: "Amber" },
];

function normalizedPercent(value: number) {
	return Math.round(Math.max(0, Math.min(1, value)) * 100);
}

function OptionalHighlightValue({
	label,
	value,
	onChange,
}: {
	label: string;
	value: number | null;
	onChange(value: number | null): void;
}) {
	return (
		<>
			<SelectField
				label={`${label} contribution`}
				ariaLabel={`${label} contribution`}
				value={value == null ? "ignore" : "configured"}
				options={[
					{ value: "ignore", label: "Ignore" },
					{ value: "configured", label: "Configured" },
				]}
				onChange={(mode) => onChange(mode === "ignore" ? null : 1)}
			/>
			{value != null && (
				<NumberField
					label={`${label} (%)`}
					min={0}
					max={100}
					step={1}
					unit="%"
					value={normalizedPercent(value)}
					onChange={(event) =>
						onChange(
							Math.max(0, Math.min(100, Number(event.target.value))) / 100,
						)
					}
				/>
			)}
		</>
	);
}

export function HighlightLookSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	const look = draft.highlight_look ?? DEFAULT_HIGHLIGHT_LOOK;
	const update = (changes: Partial<HighlightLookConfiguration>) =>
		controller.editDraft({
			...draft,
			highlight_look: { ...look, ...changes },
		});
	const compatibilityMessage =
		look.compatibility === "legacy_raw"
			? "LegacyRaw: this installation still has per-fixture raw Highlight overrides. Review the semantic look before choosing it."
			: look.compatibility === "needs_review"
				? "NeedsReview: existing raw Highlight overrides could not be translated unambiguously. Review this look before choosing it."
				: null;
	return (
		<article>
			<header>
				<b>Highlight look</b>
				<small>
					One semantic identification look for every show on this desk.
				</small>
			</header>
			<FormLayout labelPlacement="side">
				<NumberField
					label="Intensity (%)"
					required
					min={0}
					max={100}
					step={1}
					unit="%"
					value={normalizedPercent(look.intensity)}
					onChange={(event) =>
						update({
							intensity:
								Math.max(0, Math.min(100, Number(event.target.value))) / 100,
						})
					}
				/>
				<SelectField
					label="Shutter"
					ariaLabel="Shutter"
					value="open"
					disabled
					options={[{ value: "open", label: "Open where available" }]}
					onChange={() => undefined}
					description="Uses the fixture profile's authored Open function; raw maximum is never assumed."
				/>
				<SelectField
					label="Color"
					ariaLabel="Color"
					value={look.color ?? "ignore"}
					options={COLOR_OPTIONS}
					onChange={(color) =>
						update({
							color: color === "ignore" ? null : (color as HighlightLookColor),
						})
					}
				/>
				<OptionalHighlightValue
					label="Iris"
					value={look.iris}
					onChange={(iris) => update({ iris })}
				/>
				<OptionalHighlightValue
					label="Zoom"
					value={look.zoom}
					onChange={(zoom) => update({ zoom })}
				/>
				<OptionalHighlightValue
					label="Focus"
					value={look.focus}
					onChange={(focus) => update({ focus })}
				/>
				<OptionalHighlightValue
					label="Frost"
					value={look.frost}
					onChange={(frost) => update({ frost })}
				/>
			</FormLayout>
			{compatibilityMessage && (
				<div className="modal-error" role="alert">
					<p>{compatibilityMessage}</p>
					<Button onClick={() => update({ compatibility: "semantic" })}>
						Use semantic Highlight Look
					</Button>
				</div>
			)}
		</article>
	);
}

export function PatchHighlightSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	return (
		<article>
			<header>
				<b>Highlight patch</b>
				<small>
					Virtual Stage highlighting remains active regardless of this option.
				</small>
			</header>
			<FormLayout labelPlacement="side">
				<SwitchField
					label="Highlight patch selection via DMX"
					offLabel="Stage only"
					onLabel="Stage and DMX"
					checked={draft.patch_preview_highlight_dmx ?? false}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							patch_preview_highlight_dmx: event.target.checked,
						})
					}
				/>
			</FormLayout>
		</article>
	);
}

function PreloadSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	return (
		<article>
			<header>
				<b>Preload capture</b>
			</header>
			<FormLayout labelPlacement="side">
				<SwitchField
					label="Preload programmer changes"
					offLabel="Ignore"
					onLabel="Capture"
					checked={draft.preload_programmer_changes}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							preload_programmer_changes: event.target.checked,
						})
					}
				/>
				<SwitchField
					label="Preload physical playback actions"
					offLabel="Ignore"
					onLabel="Capture"
					description="Physical Flash and hardware/physical fader movements remain live and are not captured by Preload."
					checked={draft.preload_physical_playback_actions}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							preload_physical_playback_actions: event.target.checked,
						})
					}
				/>
				<SwitchField
					label="Preload virtual playback actions"
					offLabel="Ignore"
					onLabel="Capture"
					checked={draft.preload_virtual_playback_actions}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							preload_virtual_playback_actions: event.target.checked,
						})
					}
				/>
			</FormLayout>
		</article>
	);
}

function CommandLineTimingSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	return (
		<article>
			<header>
				<b>Direct value timing</b>
				<small>Explicit TIME always remains authoritative.</small>
			</header>
			<FormLayout labelPlacement="side">
				<SwitchField
					label="Direct entry uses Programmer Fade"
					offLabel="Immediate"
					onLabel="Programmer Fade"
					checked={draft.command_line_at_uses_programmer_fade ?? false}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							command_line_at_uses_programmer_fade: event.target.checked,
						})
					}
				/>
			</FormLayout>
		</article>
	);
}

function PreferencesPage({
	title,
	controller,
	children,
}: {
	title: string;
	controller: SetupWindowController;
	children: ReactNode;
}) {
	return (
		<>
			<h2>{title}</h2>
			<div className="setup-list programmer-setup-list">
				{children}
				{controller.programmerSettingsError && (
					<p className="modal-error" role="alert">
						{controller.programmerSettingsError}
					</p>
				)}
			</div>
		</>
	);
}

export function DefaultsSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<PreferencesPage title="Defaults" controller={controller}>
			{controller.defaultsTab === "record-update" && (
				<div className="defaults-record-update">
					<article>
						<header>
							<b>Record defaults</b>
							<small>Also available by holding Record.</small>
						</header>
						<RecordDefaultsFields
							settings={controller.recordSettings}
							onChange={controller.setRecordSettings}
						/>
					</article>
					<article>
						<header>
							<b>Update defaults</b>
							<small>Also available by holding Update.</small>
						</header>
						<UpdateDefaultsFields
							settings={controller.updateSettings}
							onChange={controller.setUpdateSettings}
						/>
					</article>
				</div>
			)}
			{controller.defaultsTab === "playback" && (
				<PlaybackDefaultsSettings controller={controller} />
			)}
			{controller.defaultsTab === "pools" && (
				<article>
					<PoolPaletteSettings />
				</article>
			)}
		</PreferencesPage>
	);
}

export function PlaybackDefaultsSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	return (
		<article>
			<header>
				<b>Cuelist playback defaults</b>
				<small>
					Applied when a new Cuelist playback is created. Existing Cuelists keep
					their saved behavior.
				</small>
			</header>
			<FormLayout labelPlacement="side">
				<SwitchField
					label="When fader reaches zero"
					offLabel="Keep running"
					onLabel="Turn Off"
					checked={draft.cuelist_auto_off_at_zero_default}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							cuelist_auto_off_at_zero_default: event.target.checked,
						})
					}
				/>
				<SwitchField
					label="When Flash is released"
					offLabel="Keep running"
					onLabel="Turn Off"
					checked={draft.cuelist_auto_off_flash_release_default}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							cuelist_auto_off_flash_release_default: event.target.checked,
						})
					}
				/>
				<SwitchField
					label="Start after first recording"
					offLabel="Leave Off"
					onLabel="Start at 100%"
					checked={draft.start_after_first_recording}
					onChange={(event) =>
						controller.editDraft({
							...draft,
							start_after_first_recording: event.target.checked,
						})
					}
				/>
			</FormLayout>
		</article>
	);
}

export function AttributesEncodersSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<PreferencesPage title="Attributes & encoders" controller={controller}>
			<AttributeRegistrySettings controller={controller} />
		</PreferencesPage>
	);
}

export function HighlightSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<PreferencesPage title="Highlight" controller={controller}>
			<HighlightLookSettings controller={controller} />
			<PatchHighlightSettings controller={controller} />
		</PreferencesPage>
	);
}

export function OthersSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<PreferencesPage title="Others" controller={controller}>
			<CommandLineTimingSettings controller={controller} />
			<PreloadSettings controller={controller} />
		</PreferencesPage>
	);
}
