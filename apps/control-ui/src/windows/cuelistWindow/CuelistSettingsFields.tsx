import {
	FormLayout,
	HorizontalFaderField,
	NumberField,
	SelectField,
	SwitchField,
} from "../../components/common";
import type { CuelistSettingsController } from "./useCuelistSettings";

export function CuelistSettingsFields({
	controller,
	priority,
}: {
	controller: CuelistSettingsController;
	priority: number;
}) {
	const { draft, priorityInputRef, update } = controller;
	return (
		<div className="cuelist-settings-columns">
			<section aria-labelledby="cuelist-priority-heading">
				<h3 id="cuelist-priority-heading">Priority</h3>
				<FormLayout labelPlacement="top">
					<NumberField
						label="Numeric priority"
						description="Resolves which Cuelist contribution wins before intensity HTP/LTP arbitration."
						min="-32768"
						max="32767"
						defaultValue={priority}
						ref={priorityInputRef}
					/>
					<SelectField
						label="Intensity priority mode"
						description="HTP uses the highest intensity at the winning priority. LTP uses the newest intensity there; other attributes remain LTP."
						value={draft.intensity_priority_mode ?? "htp"}
						onChange={(value) => update("intensity_priority_mode", value)}
						options={[
							{ value: "htp", label: "HTP" },
							{ value: "ltp", label: "LTP" },
						]}
					/>
				</FormLayout>
			</section>
			<RestartFields controller={controller} />
			<TimingFields controller={controller} />
		</div>
	);
}

function RestartFields({
	controller: { draft, update },
}: {
	controller: CuelistSettingsController;
}) {
	return (
		<section aria-labelledby="cuelist-restart-heading">
			<h3 id="cuelist-restart-heading">Restart behavior</h3>
			<FormLayout labelPlacement="top">
				<SelectField
					label="Wrap Around"
					description="Off stops at the final Cue. Tracking returns to Cue 1 while retaining tracked values. Reset releases tracked state before Cue 1."
					value={draft.wrap_mode ?? (draft.looped ? "tracking" : "off")}
					onChange={(value) => update("wrap_mode", value)}
					options={[
						{ value: "off", label: "Off" },
						{ value: "tracking", label: "Tracking" },
						{ value: "reset", label: "Reset" },
					]}
				/>
				<SelectField
					label="Restart mode"
					description="First Cue starts at Cue 1 after Off. Continue Current Cue restores the Cue that was current before Off."
					value={draft.restart_mode ?? "first_cue"}
					onChange={(value) => update("restart_mode", value)}
					options={[
						{ value: "first_cue", label: "First Cue" },
						{
							value: "continue_current_cue",
							label: "Continue Current Cue",
						},
					]}
				/>
			</FormLayout>
		</section>
	);
}

function TimingFields({
	controller: { draft, update },
}: {
	controller: CuelistSettingsController;
}) {
	return (
		<section aria-labelledby="cuelist-timing-heading">
			<h3 id="cuelist-timing-heading">Timing</h3>
			<FormLayout labelPlacement="top">
				<SwitchField
					label="Force Cue Timing"
					description="Uses each Cue's Fade and Delay for every value, temporarily overriding stored per-value timing without deleting it."
					checked={draft.force_cue_timing ?? false}
					onChange={(event) => update("force_cue_timing", event.target.checked)}
				/>
				<SwitchField
					label="Disable Cue Timing"
					description="Rehearsal bypass: makes Cue and per-value timing, TIME waits, and Chaser X-fade immediate without changing stored values. Chaser cadence continues; this overrides Force Cue Timing."
					checked={draft.disable_cue_timing ?? false}
					onChange={(event) =>
						update("disable_cue_timing", event.target.checked)
					}
				/>
				{draft.mode === "chaser" && (
					<ChaserFields controller={{ draft, update }} />
				)}
			</FormLayout>
		</section>
	);
}

function ChaserFields({
	controller: { draft, update },
}: {
	controller: Pick<CuelistSettingsController, "draft" | "update">;
}) {
	return (
		<>
			<SelectField
				label="Speed Group"
				description="Supplies the live BPM used by this Chaser."
				value={draft.speed_group ?? "legacy"}
				onChange={(value) =>
					update("speed_group", value === "legacy" ? null : value)
				}
				options={[
					...(draft.speed_group == null
						? [
								{
									value: "legacy" as const,
									label: `Legacy fixed step (${(draft.chaser_step_millis ?? 1_000) / 1_000} s)`,
									disabled: true,
								},
							]
						: []),
					...(["A", "B", "C", "D", "E"] as const).map((value) => ({
						value,
						label: value,
					})),
				]}
			/>
			<NumberField
				label="Speed multiplier"
				description="Multiplies the selected Speed Group rate: 0.5× is half speed and 2× is double speed."
				unit="×"
				allowDecimal
				showStepButtons={false}
				min="0.01"
				max="100"
				step="0.01"
				value={draft.speed_multiplier ?? 1}
				onChange={(event) =>
					update("speed_multiplier", Number(event.target.value))
				}
			/>
			<HorizontalFaderField
				label="Chaser X-fade"
				description="Percentage of each effective Chaser step used to fade: 0% snaps and 100% fades for the full step."
				minimum={0}
				maximum={100}
				step={1}
				value={draft.chaser_xfade_percent ?? 0}
				display={`${draft.chaser_xfade_percent ?? 0}%`}
				onChange={(value) => update("chaser_xfade_percent", Math.round(value))}
			/>
		</>
	);
}
