import { Button } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import type { CSSProperties } from "react";
import type { Cue, PlaybackSnapshot } from "../../api/types";
import type { CommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import {
	cueCommandAddress,
	cueMutationCommand,
	cueMutationLabel,
	cueMutationTarget,
} from "./cueCommandTarget";
import { cueTriggerKind, formatCueSeconds } from "./cueFormatting";

export interface CueTableEmptyState {
	title: string;
	description: string;
	icon: string;
}

export type CueTimingProgressField =
	| "triggerTime"
	| "inDelay"
	| "inFade"
	| "outDelay"
	| "outFade";

export type CueTimingProgressByRow = Record<
	number,
	Partial<Record<CueTimingProgressField, number>>
>;

export type CueEditableProperty =
	| "trigger"
	| "triggerTime"
	| "inDelay"
	| "inFade"
	| "outDelay"
	| "outFade";

function normalizedProgress(value: number | undefined): number | undefined {
	if (value === undefined || !Number.isFinite(value)) return undefined;
	return Math.min(1, Math.max(0, value));
}

function TimingCell({
	label,
	value,
	progress,
	onActivate,
}: {
	label: string;
	value: string;
	progress?: number;
	onActivate?: () => void;
}) {
	const normalized = normalizedProgress(progress);
	const style =
		normalized === undefined
			? undefined
			: ({
					"--cue-timing-progress": `${normalized * 100}%`,
				} as CSSProperties);
	return (
		<td className="cue-timing-cell">
			<Button
				type="button"
				className="cue-timing-cell-value"
				style={style}
				aria-label={label.replace(" progress", "")}
				onClick={(event) => {
					event.stopPropagation();
					onActivate?.();
				}}
				disabled={!onActivate}
			>
				{normalized === undefined ? (
					value
				) : (
					<span
						role="progressbar"
						aria-label={label}
						aria-valuemin={0}
						aria-valuemax={100}
						aria-valuenow={Math.round(normalized * 100)}
						aria-valuetext={`${value}, ${Math.round(normalized * 100)}% complete`}
					>
						{value}
					</span>
				)}
			</Button>
		</td>
	);
}

function cueTriggerLabel(cues: Cue[], cue: Cue) {
	if (cue.trigger.type !== "link") return cueTriggerKind(cue).toUpperCase();
	const destination = cues.find(
		(candidate) => candidate.id === cue.trigger.cue_id,
	);
	return destination
		? `LINK → Cue ${destination.number}${destination.name ? ` · ${destination.name}` : ""}`
		: "LINK → Missing Cue";
}

function activateCueAt({
	index,
	cues,
	mutationTarget,
	playbackNumber,
	command,
	onSelectCue,
}: {
	index: number;
	cues: Cue[];
	mutationTarget: ReturnType<typeof cueMutationTarget>;
	playbackNumber?: number | null;
	command?: Pick<CommandLineSurface, "text" | "replace" | "execute">;
	onSelectCue(index: number): void;
}) {
	const cue = cues[index];
	if (mutationTarget && playbackNumber != null && command && cue) {
		const mutation = cueMutationCommand(
			mutationTarget,
			cueCommandAddress(playbackNumber, cue.number),
		);
		if (mutation.kind === "replace") void command.replace(mutation.command);
		else void command.execute(mutation.command);
		return;
	}
	onSelectCue(index);
}

function CueTableHeader({ compactRows }: { compactRows: boolean }) {
	return (
		<thead>
			<tr>
				{!compactRows && <th className="cue-preview-column">Preview</th>}
				<th className="cue-number-column">No.</th>
				<th>Name</th>
				<th className="cue-trigger-column">Trigger</th>
				<th className="cue-timing-column">Trigger Time</th>
				<th className="cue-timing-column">In Delay</th>
				<th className="cue-timing-column">In Fade</th>
				<th className="cue-timing-column">Out Delay</th>
				<th className="cue-timing-column">Out Fade</th>
			</tr>
		</thead>
	);
}

export function CueTable({
	cues,
	active,
	selectedCue,
	settingsOpen,
	thumbnails,
	emptyState,
	onSelectCue,
	onEditCueProperty,
	interactive = true,
	compactRows = false,
	timingProgressByRow = {},
	playbackNumber,
	command,
}: {
	cues: Cue[];
	active: PlaybackSnapshot["active"][number] | undefined;
	selectedCue: number;
	settingsOpen: boolean;
	thumbnails: Record<number, string>;
	emptyState: CueTableEmptyState;
	onSelectCue: (index: number) => void;
	onEditCueProperty?: (index: number, property: CueEditableProperty) => void;
	interactive?: boolean;
	compactRows?: boolean;
	timingProgressByRow?: CueTimingProgressByRow;
	playbackNumber?: number | null;
	command?: Pick<CommandLineSurface, "text" | "replace" | "execute">;
}) {
	const mutationTarget =
		interactive && !settingsOpen && playbackNumber != null && command
			? cueMutationTarget(command.text)
			: null;
	const activateCue = (index: number) =>
		activateCueAt({
			index,
			cues,
			mutationTarget,
			playbackNumber,
			command,
			onSelectCue,
		});
	const activateProperty = (index: number, property: CueEditableProperty) => {
		if (mutationTarget) activateCue(index);
		else onEditCueProperty?.(index, property);
	};
	return (
		<div className="cue-editor">
			<WindowScrollArea
				className="cue-table-wrap"
				emptyState={cues.length ? null : emptyState}
			>
				{cues.length > 0 && (
					<table
						className={`cue-table ${compactRows ? "compact-cue-rows" : ""}`}
					>
						<CueTableHeader compactRows={compactRows} />
						<tbody>
							{cues.map((cue, index) => (
								<tr
									tabIndex={interactive ? 0 : undefined}
									aria-disabled={!interactive || settingsOpen}
									onClick={() => {
										if (interactive && !settingsOpen) activateCue(index);
									}}
									onKeyDown={(event) => {
										if (!interactive || settingsOpen) return;
										if (event.key === "Enter" || event.key === " ") {
											event.preventDefault();
											activateCue(index);
										}
									}}
									key={cue.number}
									className={`${active?.cue_index === index ? "current" : active?.effective_next_cue_number === cue.number ? "next" : ""} ${interactive && selectedCue === index ? "selected" : ""} ${mutationTarget ? `${mutationTarget.operation}-target cue-command-target` : ""}`}
								>
									{!compactRows && (
										<td className="cue-preview-column">
											{thumbnails[index] && (
												<img src={thumbnails[index]} alt="" />
											)}
										</td>
									)}
									<td>
										<b>{cue.number}</b>
										{mutationTarget && (
											<span
												className={`cue-command-target-badge ${mutationTarget.operation}`}
											>
												{cueMutationLabel(mutationTarget)}
											</span>
										)}
									</td>
									<td>
										{cue.name || `Cue ${cue.number}`}
										{Boolean(cue.dynamic_changes?.length) && (
											<small
												className="cue-dynamics-marker"
												title="Cue contains tracked Dynamic or FAT content"
												role="img"
												aria-label="Contains Dynamics"
											>
												∿
											</small>
										)}
									</td>
									<td className="cue-trigger-column">
										<Button
											type="button"
											aria-label="Trigger"
											disabled={
												!interactive || settingsOpen || !onEditCueProperty
											}
											onClick={(event) => {
												event.stopPropagation();
												activateProperty(index, "trigger");
											}}
										>
											{cueTriggerLabel(cues, cue)}
										</Button>
									</td>
									<TimingCell
										label="Trigger Time progress"
										value={
											typeof cue.trigger.delay_millis === "number"
												? formatCueSeconds(cue.trigger.delay_millis)
												: "—"
										}
										progress={timingProgressByRow[index]?.triggerTime}
										onActivate={
											interactive && !settingsOpen && onEditCueProperty
												? () => activateProperty(index, "triggerTime")
												: undefined
										}
									/>
									<TimingCell
										label="In Delay progress"
										value={formatCueSeconds(cue.delay_millis)}
										progress={timingProgressByRow[index]?.inDelay}
										onActivate={
											interactive && !settingsOpen && onEditCueProperty
												? () => activateProperty(index, "inDelay")
												: undefined
										}
									/>
									<TimingCell
										label="In Fade progress"
										value={formatCueSeconds(cue.fade_millis)}
										progress={timingProgressByRow[index]?.inFade}
										onActivate={
											interactive && !settingsOpen && onEditCueProperty
												? () => activateProperty(index, "inFade")
												: undefined
										}
									/>
									<TimingCell
										label="Out Delay progress"
										value={formatCueSeconds(
											cue.out_delay_millis ?? cue.delay_millis,
										)}
										progress={timingProgressByRow[index]?.outDelay}
										onActivate={
											interactive && !settingsOpen && onEditCueProperty
												? () => activateProperty(index, "outDelay")
												: undefined
										}
									/>
									<TimingCell
										label="Out Fade progress"
										value={formatCueSeconds(
											cue.out_fade_millis ?? cue.fade_millis,
										)}
										progress={timingProgressByRow[index]?.outFade}
										onActivate={
											interactive && !settingsOpen && onEditCueProperty
												? () => activateProperty(index, "outFade")
												: undefined
										}
									/>
								</tr>
							))}
						</tbody>
					</table>
				)}
			</WindowScrollArea>
		</div>
	);
}
