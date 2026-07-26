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
	key: "fade_millis" | "delay_millis",
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
			{(["fade_millis", "delay_millis"] as const).map((key) => (
				<NumberField
					key={key}
					ref={key === "fade_millis" ? refs.fade : refs.delay}
					keyboardRequest={
						keyboardRequests[key === "fade_millis" ? "fade" : "delay"]
					}
					label={key === "fade_millis" ? "Fade" : "Delay"}
					unit="s"
					allowDecimal
					min="0"
					value={actions.draft[key] / 1000}
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
	refs,
	keyboardRequests,
}: {
	actions: CueDraftActions;
	refs: CueFieldRefs;
	keyboardRequests: Partial<Record<CueKeyboardField, number>>;
}) {
	const kind = cueTriggerKind(actions.draft);
	const triggerMillis = Number(actions.draft.trigger.delay_millis ?? 0);
	const [triggerOpenRequest, setTriggerOpenRequest] = useState(0);
	const updateTriggerTime = (seconds: string, commit: boolean) => {
		const next = {
			...actions.draft,
			trigger: {
				type: "wait",
				delay_millis: Math.round(Number(seconds) * 1000),
			},
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
								trigger: cueTrigger(value, triggerMillis),
							};
							actions.setDraft(next);
							void actions.save(next);
						}}
						options={[
							{ value: "go", label: "GO" },
							{ value: "follow", label: "FOLLOW" },
							{ value: "time", label: "TIME" },
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
			{kind === "time" && (
				<NumberField
					ref={refs.triggerTime}
					keyboardRequest={keyboardRequests.triggerTime}
					label="Trigger time"
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
		</>
	);
}

export function CuePropertyFields({
	actions,
	refs,
	keyboardRequests = {},
}: {
	actions: CueDraftActions;
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
					refs={refs}
					keyboardRequests={keyboardRequests}
				/>
			</div>
		</FormLayout>
	);
}

export type CueKeyboardField = "title" | "fade" | "delay" | "triggerTime";
