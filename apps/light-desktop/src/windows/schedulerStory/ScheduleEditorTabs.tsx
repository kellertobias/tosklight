import {
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import type {
	PlaybackScheduleTarget,
	ScheduleDraft,
	ScheduledPlaybackAction,
} from "../../features/scheduler/contracts";

export type ScheduleEditorTab = "name" | "when" | "action";

export interface ScheduleEditorForm {
	name: string;
	enabled: boolean;
	timingType: "interval" | "calendar_expression" | "one_time";
	intervalSeconds: number;
	calendarExpression: string;
	oneTimeDate: string;
	oneTimeTime: string;
	remainEnabledAfterSuccess: boolean;
	playbackId: string;
	action: ScheduledPlaybackAction;
	setMaster: boolean;
	masterPercent: number;
	fadeSeconds: number;
}

type UpdateForm = <Key extends keyof ScheduleEditorForm>(
	key: Key,
	value: ScheduleEditorForm[Key],
) => void;

export function scheduleEditorForm(
	draft: ScheduleDraft | null,
	targets: readonly PlaybackScheduleTarget[],
	serverDate: string,
): ScheduleEditorForm {
	return {
		name: draft?.name ?? "New schedule",
		enabled: draft?.enabled ?? true,
		timingType: draft?.timing.type ?? "interval",
		intervalSeconds:
			draft?.timing.type === "interval" ? draft.timing.everySeconds : 300,
		calendarExpression:
			draft?.timing.type === "calendar_expression"
				? draft.timing.expression
				: "0 14 * * 1",
		oneTimeDate:
			draft?.timing.type === "one_time" ? draft.timing.localDate : serverDate,
		oneTimeTime:
			draft?.timing.type === "one_time" ? draft.timing.localTime : "14:00:00",
		remainEnabledAfterSuccess:
			draft?.timing.type === "one_time"
				? draft.timing.remainEnabledAfterSuccess
				: false,
		playbackId:
			draft?.target.type === "playback"
				? draft.target.playbackId
				: (targets[0]?.id ?? ""),
		action: draft?.target.type === "playback" ? draft.target.action : "go",
		setMaster:
			draft?.target.type === "playback" && draft.target.masterPercent != null,
		masterPercent:
			draft?.target.type === "playback"
				? (draft.target.masterPercent ?? 100)
				: 100,
		fadeSeconds:
			draft?.target.type === "playback"
				? (draft.target.fadeMillis ?? 0) / 1_000
				: 0,
	};
}

export function draftFromScheduleEditor(
	form: ScheduleEditorForm,
	targets: readonly PlaybackScheduleTarget[],
): ScheduleDraft | null {
	const target = targets.find((candidate) => candidate.id === form.playbackId);
	if (!target) return null;
	return {
		name: form.name.trim(),
		enabled: form.enabled,
		timing:
			form.timingType === "interval"
				? {
						type: "interval",
						everySeconds: Math.max(1, Math.round(form.intervalSeconds)),
						anchor: "activation",
					}
				: form.timingType === "calendar_expression"
					? {
							type: "calendar_expression",
							expression: form.calendarExpression.trim(),
							summary: "",
						}
					: {
							type: "one_time",
							localDate: form.oneTimeDate,
							localTime: form.oneTimeTime,
							remainEnabledAfterSuccess: form.remainEnabledAfterSuccess,
						},
		target: {
			type: "playback",
			playbackId: target.id,
			label: target.label,
			page: target.page,
			slot: target.slot,
			playback: target.playback,
			action: form.action,
			masterPercent:
				form.setMaster && target.supportsMaster
					? Math.max(0, Math.min(100, form.masterPercent))
					: null,
			fadeMillis:
				form.setMaster && target.supportsMaster
					? Math.max(0, Math.round(form.fadeSeconds * 1_000))
					: null,
		},
	};
}

export function ScheduleEditorTabs({
	activeTab,
	form,
	timezone,
	playbackTargets,
	update,
}: {
	activeTab: ScheduleEditorTab;
	form: ScheduleEditorForm;
	timezone: string;
	playbackTargets: readonly PlaybackScheduleTarget[];
	update: UpdateForm;
}) {
	if (activeTab === "name")
		return (
			<section className="scheduler-editor-section">
				<FormLayout columns={2}>
					<TextField
						label="Name"
						value={form.name}
						onChange={(event) => update("name", event.target.value)}
					/>
					<SwitchField
						label="Enabled"
						offLabel="Disabled"
						onLabel="Enabled"
						checked={form.enabled}
						onChange={(event) => update("enabled", event.target.checked)}
					/>
				</FormLayout>
			</section>
		);
	if (activeTab === "when")
		return <WhenFields form={form} timezone={timezone} update={update} />;
	return (
		<ActionFields
			form={form}
			playbackTargets={playbackTargets}
			update={update}
		/>
	);
}

function WhenFields({
	form,
	timezone,
	update,
}: {
	form: ScheduleEditorForm;
	timezone: string;
	update: UpdateForm;
}) {
	return (
		<section className="scheduler-editor-section">
			<SelectField
				label="Trigger type"
				value={form.timingType}
				options={[
					{ value: "interval", label: "Interval" },
					{
						value: "calendar_expression",
						label: "Calendar expression",
					},
					{ value: "one_time", label: "One-time" },
				]}
				onChange={(value) =>
					update("timingType", value as ScheduleEditorForm["timingType"])
				}
			/>
			<p className="scheduler-timezone" role="status">
				Authoritative server timezone:{" "}
				<strong>{timezone || "Unavailable"}</strong>
			</p>
			{form.timingType === "interval" && (
				<>
					<NumberField
						label="Interval"
						min={1}
						unit="seconds"
						value={form.intervalSeconds}
						onChange={(event) =>
							update("intervalSeconds", Number(event.target.value) || 1)
						}
					/>
					<small>
						The interval is anchored when this Schedule becomes active. The
						server validates the minimum duration.
					</small>
				</>
			)}
			{form.timingType === "calendar_expression" && (
				<TextField
					label="Calendar expression"
					value={form.calendarExpression}
					description="Server-supported five-field calendar expression"
					onChange={(event) => update("calendarExpression", event.target.value)}
				/>
			)}
			{form.timingType === "one_time" && (
				<>
					<FormLayout columns={2}>
						<TextField
							label="Date"
							value={form.oneTimeDate}
							description="YYYY-MM-DD"
							onChange={(event) => update("oneTimeDate", event.target.value)}
						/>
						<TextField
							label="Time"
							value={form.oneTimeTime}
							description="HH:MM:SS"
							onChange={(event) => update("oneTimeTime", event.target.value)}
						/>
					</FormLayout>
					<SwitchField
						label="After successful occurrence"
						offLabel="Disable Schedule"
						onLabel="Keep enabled as history"
						checked={form.remainEnabledAfterSuccess}
						onChange={(event) =>
							update("remainEnabledAfterSuccess", event.target.checked)
						}
					/>
				</>
			)}
		</section>
	);
}

function ActionFields({
	form,
	playbackTargets,
	update,
}: {
	form: ScheduleEditorForm;
	playbackTargets: readonly PlaybackScheduleTarget[];
	update: UpdateForm;
}) {
	const target = playbackTargets.find(
		(candidate) => candidate.id === form.playbackId,
	);
	const supportedActions = target?.supportedActions ?? [];
	return (
		<section className="scheduler-editor-section">
			<SelectField
				label="Action family"
				value="playback"
				options={[
					{ value: "playback", label: "Playback" },
					{ value: "macro", label: "Start Macro (unavailable)" },
				]}
				onChange={() => undefined}
			/>
			<p className="scheduler-unavailable">
				Macro scheduling becomes available when the Macro runtime is installed.
			</p>
			<SelectField
				label="Playback"
				value={form.playbackId}
				options={playbackTargets.map((candidate) => ({
					value: candidate.id,
					label: `Page ${candidate.page} · Slot ${candidate.slot} · Playback ${candidate.playback} · ${candidate.label}`,
				}))}
				onChange={(value) => {
					const next = playbackTargets.find(
						(candidate) => candidate.id === value,
					);
					update("playbackId", value);
					if (next && !next.supportedActions.includes(form.action))
						update("action", next.supportedActions[0] ?? "go");
				}}
			/>
			<SelectField
				label="Playback action"
				value={form.action}
				options={supportedActions.map((action) => ({
					value: action,
					label: playbackActionLabel(action),
				}))}
				onChange={(value) => update("action", value as ScheduledPlaybackAction)}
			/>
			<SwitchField
				label="Set playback master"
				offLabel="Leave unchanged"
				onLabel="Set master"
				checked={form.setMaster}
				disabled={!target?.supportsMaster}
				onChange={(event) => update("setMaster", event.target.checked)}
			/>
			{form.setMaster && target?.supportsMaster && (
				<FormLayout columns={2}>
					<NumberField
						label="Master"
						min={0}
						max={100}
						unit="%"
						value={form.masterPercent}
						onChange={(event) =>
							update("masterPercent", Number(event.target.value) || 0)
						}
					/>
					<NumberField
						label="Fade time"
						min={0}
						unit="s"
						value={form.fadeSeconds}
						onChange={(event) =>
							update("fadeSeconds", Number(event.target.value) || 0)
						}
					/>
				</FormLayout>
			)}
		</section>
	);
}

export function playbackActionLabel(action: ScheduledPlaybackAction) {
	return {
		go: "Go",
		pause: "Pause",
		on: "On",
		off: "Off",
		release: "Release",
		toggle: "Toggle",
	}[action];
}
