import {
	GroupedSelectionModal,
	InputModal,
	ModalNumberEditor,
} from "@tosklight/ui";
import { useState } from "react";
import type { Cue } from "../../api/types";
import {
	useReleaseFadeMillis,
	useSequenceMasterFadeMillis,
} from "../../features/configuration/ConfigurationState";
import type { CueEditableProperty } from "./CueTable";
import {
	cueJump,
	cueTrigger,
	cueTriggerKind,
	withCueJump,
} from "./cueFormatting";

const propertyLabels: Record<CueEditableProperty, string> = {
	name: "Cue Name",
	information: "Cue Information",
	jump: "Jump",
	jumpCount: "Jump Count",
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

type TimingProperty = keyof typeof timingKeys;
type NumberProperty = "jumpCount" | "triggerTime" | TimingProperty;
type TriggerKind = "go" | "follow" | "time" | "timecode" | "link";
type TriggerChoice = Exclude<TriggerKind, "link"> | `link:${string}`;

function timingValue(
	cue: Cue,
	property: TimingProperty,
	releaseFadeMillis: number,
	sequenceMasterFadeMillis: number,
) {
	if (property === "outFade" && cue.out_fade_link === "release")
		return releaseFadeMillis / 1000;
	if (property === "outDelay" && cue.out_delay_link === "in_fade")
		return (cue.fade_millis || sequenceMasterFadeMillis) / 1000;
	const fallback = property === "outFade" ? cue.fade_millis : cue.delay_millis;
	return Number(cue[timingKeys[property]] ?? fallback) / 1000;
}

function numberValue(
	cue: Cue,
	property: NumberProperty,
	releaseFadeMillis: number,
	sequenceMasterFadeMillis: number,
): string {
	if (property === "jumpCount") return String(cueJump(cue)?.count ?? 1);
	if (property === "triggerTime")
		return String(Number(cue.trigger.delay_millis ?? 0) / 1000);
	return String(
		timingValue(cue, property, releaseFadeMillis, sequenceMasterFadeMillis),
	);
}

function cueWithNumber(
	cue: Cue,
	property: NumberProperty,
	value: string,
): Cue | null {
	const parsed = Number(value);
	if (!Number.isFinite(parsed) || parsed < 0) return null;
	if (property === "jumpCount") {
		const jump = cueJump(cue);
		if (!jump || !Number.isSafeInteger(parsed) || parsed < 1) return null;
		return withCueJump(cue, { ...jump, count: parsed });
	}
	const millis = Math.round(parsed * 1000);
	if (!Number.isSafeInteger(millis)) return null;
	if (property === "triggerTime") {
		const kind = cueTriggerKind(cue);
		return {
			...cue,
			trigger:
				kind === "link"
					? cueTrigger("link", millis, String(cue.trigger.cue_id ?? ""))
					: cueTrigger("time", millis),
		};
	}
	if (property === "outFade")
		return { ...cue, out_fade_millis: millis, out_fade_link: undefined };
	if (property === "outDelay")
		return { ...cue, out_delay_millis: millis, out_delay_link: undefined };
	return { ...cue, [timingKeys[property]]: millis };
}

function cueWithTrigger(
	cue: Cue,
	kind: TriggerKind,
	linkedCueId?: string,
): Cue {
	const triggerMillis = Number(cue.trigger.delay_millis ?? 0);
	return {
		...cue,
		trigger: cueTrigger(
			kind,
			triggerMillis,
			kind === "link" ? linkedCueId : undefined,
			Number(cue.trigger.frame ?? 0),
		),
	};
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
	const label = propertyLabels[property];
	const releaseFadeMillis = useReleaseFadeMillis() ?? 3_000;
	const sequenceMasterFadeMillis = useSequenceMasterFadeMillis() ?? 3_000;
	const effectiveInFadeMillis = cue.fade_millis || sequenceMasterFadeMillis;
	const [numberDraft, setNumberDraft] = useState(() =>
		property === "name" ||
		property === "information" ||
		property === "jump" ||
		property === "trigger"
			? ""
			: numberValue(cue, property, releaseFadeMillis, sequenceMasterFadeMillis),
	);
	const [localError, setLocalError] = useState("");
	const [busy, setBusy] = useState(false);
	const error = localError || editError;
	const save = async (next: Cue) => {
		if (busy) return;
		setBusy(true);
		setLocalError("");
		try {
			if (await onSave(next)) onCancel();
		} finally {
			setBusy(false);
		}
	};

	if (property === "name" || property === "information") {
		const value = property === "name" ? cue.name : (cue.information ?? "");
		return (
			<InputModal
				kind={property === "information" ? "multiline" : "text"}
				label={label}
				value={value}
				initialCaret={value.length}
				error={error}
				onCommit={(next) =>
					void save(
						property === "name"
							? { ...cue, name: next }
							: { ...cue, information: next },
					)
				}
				onCancel={onCancel}
			/>
		);
	}

	if (property === "jump") {
		const jump = cueJump(cue);
		return (
			<GroupedSelectionModal
				ariaLabel={label}
				title={`${label} · Cue ${cue.number}`}
				closeLabel={`Close ${label}`}
				value={jump?.cue_id ?? ""}
				groups={[
					{
						label: "Cue",
						options: cues
							.filter((candidate) => candidate.id)
							.map((candidate) => ({
								value: candidate.id as string,
								label: `Cue ${candidate.number}${candidate.name ? ` · ${candidate.name}` : ""}`,
								description: "Jump to this Cue when the current Cue completes.",
							})),
					},
				]}
				error={error}
				clearAction={{ label: "No Jump", value: "" }}
				onChange={(cueId) =>
					void save(
						withCueJump(
							cue,
							cueId
								? {
										type: "jump",
										cue_id: cueId,
										count: jump?.count ?? 1,
									}
								: null,
						),
					)
				}
				onClose={onCancel}
			/>
		);
	}

	if (property === "trigger") {
		const linkCandidates = cues.filter(
			(candidate) => candidate.id && candidate.id !== cue.id,
		);
		const options: Array<{
			value: TriggerChoice;
			label: string;
			description: string;
		}> = [
			{ value: "go", label: "GO", description: "Wait for an explicit GO." },
			{
				value: "follow",
				label: "FOLLOW",
				description: "Continue after the Cue fade completes.",
			},
			{
				value: "time",
				label: "TIME",
				description: "Continue after the stored Trigger Time.",
			},
			{
				value: "timecode",
				label: "TIMECODE",
				description: "Run at the stored Timecode frame.",
			},
		];
		const linkOptions = linkCandidates.map((candidate) => ({
			value: `link:${candidate.id}` as const,
			label: `LINK → Cue ${candidate.number}${candidate.name ? ` · ${candidate.name}` : ""}`,
			description: "Follow this Cue by its stable identity.",
		}));
		const selectedTrigger =
			cueTriggerKind(cue) === "link"
				? (`link:${String(cue.trigger.cue_id ?? "")}` as const)
				: (cueTriggerKind(cue) as TriggerChoice);
		return (
			<GroupedSelectionModal
				ariaLabel={label}
				title={`${label} · Cue ${cue.number}`}
				closeLabel={`Close ${label}`}
				value={selectedTrigger}
				groups={[
					{ label: "Trigger type", options },
					...(linkOptions.length
						? [{ label: "Link destination", options: linkOptions }]
						: []),
				]}
				error={error}
				onChange={(choice) => {
					const linkedCueId = choice.startsWith("link:")
						? choice.slice("link:".length)
						: undefined;
					const kind: TriggerKind = linkedCueId
						? "link"
						: (choice as Exclude<TriggerKind, "link">);
					void save(cueWithTrigger(cue, kind, linkedCueId));
				}}
				onClose={onCancel}
			/>
		);
	}

	const invalidMessage =
		property === "jumpCount"
			? "Enter a whole Jump Count of one or greater."
			: "Enter a time of zero or greater.";
	const linkedSource =
		property === "outFade" && cue.out_fade_link === "release"
			? { label: "Release", millis: releaseFadeMillis }
			: property === "outDelay" && cue.out_delay_link === "in_fade"
				? { label: "In Fade", millis: effectiveInFadeMillis }
				: null;
	const linkAction =
		property === "outFade"
			? {
					label: linkedSource ? "Use explicit value" : "Use Release",
					next: {
						...cue,
						out_fade_millis:
							linkedSource && cue.out_fade_millis == null
								? releaseFadeMillis
								: cue.out_fade_millis,
						out_fade_link: linkedSource ? undefined : ("release" as const),
					},
				}
			: property === "outDelay"
				? {
						label: linkedSource ? "Use explicit value" : "Use In Fade",
						next: {
							...cue,
							out_delay_millis:
								linkedSource && cue.out_delay_millis == null
									? effectiveInFadeMillis
									: cue.out_delay_millis,
							out_delay_link: linkedSource ? undefined : ("in_fade" as const),
						},
					}
				: null;
	return (
		<ModalNumberEditor
			ariaLabel={label}
			title={`${label} · Cue ${cue.number}`}
			value={numberDraft}
			onChange={(value) => {
				setNumberDraft(value);
				setLocalError("");
			}}
			onSubmit={(value = numberDraft) => {
				const next = cueWithNumber(cue, property, value);
				if (!next) {
					setLocalError(invalidMessage);
					return;
				}
				void save(next);
			}}
			onClose={onCancel}
			allowDecimal={property !== "jumpCount"}
			unit={property === "jumpCount" ? undefined : "s"}
			beforeTitle={
				error || linkedSource ? (
					<>
						{linkedSource && (
							<span className="cue-timing-link-status">
								Linked to {linkedSource.label} ·{" "}
								{(linkedSource.millis / 1000).toFixed(1)} s
							</span>
						)}
						{error && (
							<span className="ui-field-error" role="alert">
								{error}
							</span>
						)}
					</>
				) : undefined
			}
			onRelease={linkAction ? () => void save(linkAction.next) : undefined}
			releaseLabel={linkAction?.label}
		/>
	);
}
