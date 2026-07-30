import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import type {
	HighlightLookColor,
	HighlightLookConfiguration,
} from "../../api/types";
import {
	RecordDefaultsFields,
	UpdateDefaultsFields,
} from "../../components/setup/ProgrammerDefaults";
import { AttributeRegistrySettings } from "./AttributeRegistrySettings";
import type { SetupWindowController } from "./controller";

const DEFAULT_HIGHLIGHT_LOOK: HighlightLookConfiguration = {
	intensity: 1,
	color: null,
	iris: null,
	zoom: null,
	focus: null,
	frost: null,
	compatibility: "needs_review",
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
				<b>Highlight Look</b>
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
			{look.compatibility === "semantic" &&
				draft.highlight_look_feedback != null &&
				draft.highlight_look_feedback.length > 0 && (
					<div className="modal-error" role="status">
						<p>
							Unsupported Highlight parts stay unchanged for these fixtures:
						</p>
						<ul>
							{draft.highlight_look_feedback.map((warning) => (
								<li key={warning}>{warning}</li>
							))}
						</ul>
					</div>
				)}
		</article>
	);
}

function PatchHighlightSettings({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const { draft } = controller;
	if (!draft) return null;
	return (
		<article>
			<header>
				<b>Show Patch</b>
				<small>
					Virtual Stage highlighting remains active regardless of this option.
				</small>
			</header>
			<FormLayout labelPlacement="side">
				<SwitchField
					label="Highlight patch selection via DMX"
					offLabel="Stage only"
					onLabel="Stage + DMX"
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
				<b>Command line timing</b>
				<small>Explicit TIME always remains authoritative.</small>
			</header>
			<FormLayout labelPlacement="side">
				<SwitchField
					label="AT uses Programmer Fade"
					offLabel="Immediate"
					onLabel="Programmer Fade"
					checked={draft.command_line_at_uses_programmer_fade ?? true}
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

export function ProgrammerSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<>
			<h2>Programmer</h2>
			<div className="setup-list programmer-setup-list">
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
				<AttributeRegistrySettings controller={controller} />
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
				<HighlightLookSettings controller={controller} />
				<PatchHighlightSettings controller={controller} />
				<CommandLineTimingSettings controller={controller} />
				<h3 className="setup-subsection-title">Preload</h3>
				<PreloadSettings controller={controller} />
				{controller.programmerSettingsError && (
					<p className="modal-error" role="alert">
						{controller.programmerSettingsError}
					</p>
				)}
			</div>
		</>
	);
}
