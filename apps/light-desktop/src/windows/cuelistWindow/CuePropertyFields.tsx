import {
	Button,
	FormField,
	FormLayout,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import {
	type Dispatch,
	type RefObject,
	type SetStateAction,
	useState,
} from "react";
import type { Cue } from "../../api/types";
import { cueTrigger, cueTriggerKind } from "./cueFormatting";

export interface CueFieldRefs {
	title: RefObject<HTMLInputElement | null>;
	fade: RefObject<HTMLInputElement | null>;
	delay: RefObject<HTMLInputElement | null>;
	outFade: RefObject<HTMLInputElement | null>;
	outDelay: RefObject<HTMLInputElement | null>;
	triggerTime: RefObject<HTMLInputElement | null>;
	triggerPicker: RefObject<HTMLDivElement | null>;
	grid: RefObject<HTMLDivElement | null>;
}

export interface CueDraftActions {
	draft: Cue;
	setDraft: Dispatch<SetStateAction<Cue | null>>;
	save: (cue: Cue) => Promise<void>;
}

function updateMillis(
	actions: CueDraftActions,
	key: "fade_millis" | "delay_millis" | "out_fade_millis" | "out_delay_millis",
	seconds: string,
	commit: boolean,
) {
	const next = { ...actions.draft, [key]: Math.round(Number(seconds) * 1000) };
	actions.setDraft(next);
	if (commit) void actions.save(next);
}

function CueTimingFields({
	actions,
	refs,
	keyboardRequests,
}: {
	actions: CueDraftActions;
	refs: CueFieldRefs;
	keyboardRequests: Partial<Record<CueKeyboardField, number>>;
}) {
	return (
		<>
			{(
				[
					["delay_millis", "In Delay", refs.delay, "delay"],
					["fade_millis", "In Fade", refs.fade, "fade"],
					["out_delay_millis", "Out Delay", refs.outDelay, "outDelay"],
					["out_fade_millis", "Out Fade", refs.outFade, "outFade"],
				] as const
			).map(([key, label, ref, keyboardField]) => (
				<NumberField
					key={key}
					ref={ref}
					keyboardRequest={keyboardRequests[keyboardField]}
					label={label}
					unit="s"
					allowDecimal
					min="0"
					value={
						(actions.draft[key] ??
							(key === "out_fade_millis"
								? actions.draft.fade_millis
								: actions.draft.delay_millis)) / 1000
					}
					onKeyboardCommit={(value) => updateMillis(actions, key, value, true)}
					onChange={(event) =>
						updateMillis(actions, key, event.target.value, false)
					}
					onBlur={(event) =>
						updateMillis(actions, key, event.currentTarget.value, true)
					}
					onKeyDown={(event) => {
						if (event.key === "Enter")
							updateMillis(actions, key, event.currentTarget.value, true);
					}}
				/>
			))}
		</>
	);
}

function CueTriggerFields({
	actions,
	cues,
	refs,
	keyboardRequests,
}: {
	actions: CueDraftActions;
	cues: Cue[];
	refs: CueFieldRefs;
	keyboardRequests: Partial<Record<CueKeyboardField, number>>;
}) {
	const kind = cueTriggerKind(actions.draft);
	const triggerMillis = Number(actions.draft.trigger.delay_millis ?? 0);
	const linkCandidates = cues.filter(
		(cue) => cue.id && cue.id !== actions.draft.id,
	);
	const linkedCueId = String(actions.draft.trigger.cue_id ?? "");
	const timecodeFrame = Number(actions.draft.trigger.frame ?? 0);
	const [triggerOpenRequest, setTriggerOpenRequest] = useState(0);
	const updateTriggerTime = (seconds: string, commit: boolean) => {
		const next = {
			...actions.draft,
			trigger:
				kind === "link"
					? cueTrigger("link", Math.round(Number(seconds) * 1000), linkedCueId)
					: cueTrigger("time", Math.round(Number(seconds) * 1000)),
		};
		actions.setDraft(next);
		if (commit) void actions.save(next);
	};
	return (
		<>
			<FormField label="Trigger">
				<div className="cue-trigger-grid-control" ref={refs.triggerPicker}>
					<SelectField
						openRequest={triggerOpenRequest}
						value={kind}
						onChange={(value) => {
							const next = {
								...actions.draft,
								trigger: cueTrigger(
									value,
									triggerMillis,
									linkedCueId || linkCandidates[0]?.id,
									timecodeFrame,
								),
							};
							actions.setDraft(next);
							void actions.save(next);
						}}
						options={[
							{ value: "go", label: "GO" },
							{ value: "follow", label: "FOLLOW" },
							{ value: "time", label: "TIME" },
							{ value: "timecode", label: "TIMECODE" },
							...(linkCandidates.length > 0
								? [{ value: "link" as const, label: "LINK" }]
								: []),
						]}
					/>
					<Button
						size="compact"
						iconOnly
						aria-label="Open Trigger picker"
						onClick={() => setTriggerOpenRequest((request) => request + 1)}
					>
						<span className="ui-keyboard-icon" aria-hidden="true">
							⌨
						</span>
					</Button>
				</div>
			</FormField>
			{kind === "link" && (
				<SelectField
					label="Link Cue"
					value={linkedCueId}
					onChange={(cueId) => {
						const next = {
							...actions.draft,
							trigger: cueTrigger("link", triggerMillis, cueId),
						};
						actions.setDraft(next);
						void actions.save(next);
					}}
					options={linkCandidates.map((cue) => ({
						value: cue.id as string,
						label: `Cue ${cue.number}${cue.name ? ` · ${cue.name}` : ""}`,
					}))}
				/>
			)}
			{(kind === "time" || kind === "link") && (
				<NumberField
					ref={refs.triggerTime}
					keyboardRequest={keyboardRequests.triggerTime}
					label={kind === "link" ? "Link delay" : "Trigger time"}
					unit="s"
					allowDecimal
					min="0"
					value={triggerMillis / 1000}
					onKeyboardCommit={(value) => updateTriggerTime(value, true)}
					onChange={(event) => updateTriggerTime(event.target.value, false)}
					onBlur={(event) => updateTriggerTime(event.currentTarget.value, true)}
					onKeyDown={(event) => {
						if (event.key === "Enter")
							updateTriggerTime(event.currentTarget.value, true);
					}}
				/>
			)}
			{kind === "timecode" && (
				<NumberField
					label="Timecode frame"
					min="0"
					value={timecodeFrame}
					onChange={(event) => {
						const next = {
							...actions.draft,
							trigger: cueTrigger(
								"timecode",
								0,
								undefined,
								Math.max(0, Math.round(Number(event.target.value))),
							),
						};
						actions.setDraft(next);
					}}
					onBlur={(event) => {
						const next = {
							...actions.draft,
							trigger: cueTrigger(
								"timecode",
								0,
								undefined,
								Math.max(0, Math.round(Number(event.currentTarget.value))),
							),
						};
						actions.setDraft(next);
						void actions.save(next);
					}}
				/>
			)}
		</>
	);
}

export function CuePropertyFields({
	actions,
	cues,
	refs,
	keyboardRequests = {},
}: {
	actions: CueDraftActions;
	cues: Cue[];
	refs: CueFieldRefs;
	keyboardRequests?: Partial<Record<CueKeyboardField, number>>;
}) {
	return (
		<FormLayout
			labelPlacement="side"
			labelWidth={62}
			className="cue-settings-grid"
		>
			<div ref={refs.grid} className="cue-settings-grid-measure">
				<TextField
					ref={refs.title}
					keyboardRequest={keyboardRequests.title}
					label="Title"
					value={actions.draft.name}
					onChange={(event) =>
						actions.setDraft({ ...actions.draft, name: event.target.value })
					}
					onKeyboardCommit={(value) => {
						const next = { ...actions.draft, name: value };
						actions.setDraft(next);
						void actions.save(next);
					}}
					onBlur={(event) =>
						void actions.save({
							...actions.draft,
							name: event.currentTarget.value,
						})
					}
					onKeyDown={(event) => {
						if (event.key === "Enter")
							void actions.save({
								...actions.draft,
								name: event.currentTarget.value,
							});
					}}
				/>
				<CueTimingFields
					actions={actions}
					refs={refs}
					keyboardRequests={keyboardRequests}
				/>
				<CueTriggerFields
					actions={actions}
					cues={cues}
					refs={refs}
					keyboardRequests={keyboardRequests}
				/>
			</div>
		</FormLayout>
	);
}

export type CueKeyboardField =
	| "title"
	| "fade"
	| "delay"
	| "outFade"
	| "outDelay"
	| "triggerTime";
