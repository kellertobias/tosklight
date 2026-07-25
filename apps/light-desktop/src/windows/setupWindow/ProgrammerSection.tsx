import { FormLayout, SwitchField } from "../../components/common";
import {
	RecordDefaultsFields,
	UpdateDefaultsFields,
} from "../../components/setup/ProgrammerDefaults";
import type { SetupWindowController } from "./controller";

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
