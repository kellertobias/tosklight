import {
	Button,
	ModalPortal,
	ModalTitleBar,
	NumberField,
	SelectField,
} from "@tosklight/ui";
import { type Dispatch, type SetStateAction, useState } from "react";
import type { Cue } from "../../api/types";
import type { CueEditableProperty } from "./CueTable";
import { cueTrigger, cueTriggerKind } from "./cueFormatting";

const propertyLabels: Record<CueEditableProperty, string> = {
	trigger: "Trigger",
	triggerTime: "Trigger Time",
	inDelay: "In Delay",
	inFade: "In Fade",
	outDelay: "Out Delay",
	outFade: "Out Fade",
};

const timingKeys = {
	inDelay: "delay_millis",
	inFade: "fade_millis",
	outDelay: "out_delay_millis",
	outFade: "out_fade_millis",
} as const;

type TimingProperty = Exclude<CueEditableProperty, "trigger" | "triggerTime">;

function timingValue(cue: Cue, property: TimingProperty) {
	const fallback = property === "outFade" ? cue.fade_millis : cue.delay_millis;
	return Number(cue[timingKeys[property]] ?? fallback) / 1000;
}

function validCueDraft(cue: Cue, property: CueEditableProperty) {
	const triggerMillis = Number(cue.trigger.delay_millis ?? 0);
	return [
		cue.fade_millis,
		cue.delay_millis,
		cue.out_fade_millis ?? 0,
		cue.out_delay_millis ?? 0,
		property === "triggerTime" ? triggerMillis : 0,
	].every((value) => Number.isSafeInteger(value) && value >= 0);
}

function CueTriggerPropertyField({
	draft,
	setDraft,
	cues,
}: {
	draft: Cue;
	setDraft: Dispatch<SetStateAction<Cue>>;
	cues: Cue[];
}) {
	const kind = cueTriggerKind(draft);
	const linkCandidates = cues.filter(
		(candidate) => candidate.id && candidate.id !== draft.id,
	);
	const linkedCueId = String(
		draft.trigger.cue_id ?? linkCandidates[0]?.id ?? "",
	);
	const triggerMillis = Number(draft.trigger.delay_millis ?? 0);
	return (
		<>
			<SelectField
				label="Trigger"
				value={kind}
				onChange={(value) =>
					setDraft({
						...draft,
						trigger: cueTrigger(
							value,
							triggerMillis,
							linkedCueId,
							Number(draft.trigger.frame ?? 0),
						),
					})
				}
				options={[
					{ value: "go", label: "GO" },
					{ value: "follow", label: "FOLLOW" },
					{ value: "time", label: "TIME" },
					{ value: "timecode", label: "TIMECODE" },
					...(linkCandidates.length
						? [{ value: "link" as const, label: "LINK" }]
						: []),
				]}
			/>
			{kind === "link" && (
				<SelectField
					label="Link Cue"
					value={linkedCueId}
					onChange={(cueId) =>
						setDraft({
							...draft,
							trigger: cueTrigger("link", triggerMillis, cueId),
						})
					}
					options={linkCandidates.map((candidate) => ({
						value: candidate.id as string,
						label: `Cue ${candidate.number}${candidate.name ? ` · ${candidate.name}` : ""}`,
					}))}
				/>
			)}
			{kind === "timecode" && (
				<NumberField
					label="Timecode frame"
					min="0"
					value={Number(draft.trigger.frame ?? 0)}
					onChange={(event) =>
						setDraft({
							...draft,
							trigger: cueTrigger(
								"timecode",
								0,
								undefined,
								Math.max(0, Math.round(Number(event.target.value))),
							),
						})
					}
				/>
			)}
		</>
	);
}

function CueTimingPropertyField({
	draft,
	setDraft,
	property,
	label,
	onEnter,
}: {
	draft: Cue;
	setDraft: Dispatch<SetStateAction<Cue>>;
	property: Exclude<CueEditableProperty, "trigger">;
	label: string;
	onEnter: () => void;
}) {
	const kind = cueTriggerKind(draft);
	const linkedCueId = String(draft.trigger.cue_id ?? "");
	const triggerMillis = Number(draft.trigger.delay_millis ?? 0);
	return (
		<NumberField
			autoFocus
			label={label}
			unit="s"
			allowDecimal
			min="0"
			value={
				property === "triggerTime"
					? triggerMillis / 1000
					: timingValue(draft, property)
			}
			onChange={(event) => {
				const millis = Math.round(Number(event.target.value) * 1000);
				if (property === "triggerTime") {
					setDraft({
						...draft,
						trigger:
							kind === "link"
								? cueTrigger("link", millis, linkedCueId)
								: cueTrigger("time", millis),
					});
				} else setDraft({ ...draft, [timingKeys[property]]: millis });
			}}
			onKeyDown={(event) => event.key === "Enter" && onEnter()}
		/>
	);
}

export function CuePropertyModal({
	cue,
	cues,
	property,
	editError,
	onCancel,
	onSave,
}: {
	cue: Cue;
	cues: Cue[];
	property: CueEditableProperty;
	editError: string;
	onCancel: () => void;
	onSave: (cue: Cue) => Promise<boolean>;
}) {
	const [draft, setDraft] = useState(cue);
	const [busy, setBusy] = useState(false);
	const label = propertyLabels[property];
	const valid = validCueDraft(draft, property);
	const save = async () => {
		if (!valid || busy) return;
		setBusy(true);
		try {
			if (await onSave(draft)) onCancel();
		} finally {
			setBusy(false);
		}
	};
	return (
		<ModalPortal onClose={onCancel}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onCancel()
				}
			>
				<section
					className="nested-modal cue-property-modal"
					role="dialog"
					aria-modal="true"
					aria-label={label}
				>
					<ModalTitleBar
						title={`${label} · Cue ${cue.number}`}
						closeLabel={`Cancel ${label}`}
						onClose={onCancel}
					/>
					<div className="cue-property-modal-body">
						{property === "trigger" ? (
							<CueTriggerPropertyField
								draft={draft}
								setDraft={setDraft}
								cues={cues}
							/>
						) : (
							<CueTimingPropertyField
								draft={draft}
								setDraft={setDraft}
								property={property}
								label={label}
								onEnter={() => void save()}
							/>
						)}
						{!valid && (
							<p className="ui-field-error" role="alert">
								Enter a time of zero or greater.
							</p>
						)}
						{editError && (
							<p className="ui-field-error" role="alert">
								{editError}
							</p>
						)}
					</div>
					<footer className="cue-property-modal-actions">
						<Button disabled={busy} onClick={onCancel}>
							Cancel
						</Button>
						<Button
							className="primary"
							disabled={!valid || busy}
							onClick={() => void save()}
						>
							Save
						</Button>
					</footer>
				</section>
			</div>
		</ModalPortal>
	);
}
