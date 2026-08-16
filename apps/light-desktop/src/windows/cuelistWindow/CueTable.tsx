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
import { cueJump, cueTriggerKind, formatCueSeconds } from "./cueFormatting";

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
	| "name"
	| "information"
	| "jump"
	| "jumpCount"
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

function cueJumpLabel(cues: Cue[], cue: Cue) {
	const jump = cueJump(cue);
	if (!jump) return "—";
	const destination = cues.find((candidate) => candidate.id === jump.cue_id);
	return destination
		? `Cue ${destination.number}${destination.name ? ` · ${destination.name}` : ""}`
		: "Missing Cue";
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
				<th className="cue-information-column">Info</th>
				<th className="cue-jump-column">Jump</th>
				<th className="cue-jump-count-column">Jump Count</th>
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

function CueTimingCells({
	cue,
	progress,
	editable,
	onActivate,
}: {
	cue: Cue;
	progress: Partial<Record<CueTimingProgressField, number>> | undefined;
	editable: boolean;
	onActivate: (property: CueEditableProperty) => void;
}) {
	const activate = (property: CueEditableProperty) =>
		editable ? () => onActivate(property) : undefined;
	return (
		<>
			<TimingCell
				label="Trigger Time progress"
				value={
					typeof cue.trigger.delay_millis === "number"
						? formatCueSeconds(cue.trigger.delay_millis)
						: "—"
				}
				progress={progress?.triggerTime}
				onActivate={activate("triggerTime")}
			/>
			<TimingCell
				label="In Delay progress"
				value={formatCueSeconds(cue.delay_millis)}
				progress={progress?.inDelay}
				onActivate={activate("inDelay")}
			/>
			<TimingCell
				label="In Fade progress"
				value={formatCueSeconds(cue.fade_millis)}
				progress={progress?.inFade}
				onActivate={activate("inFade")}
			/>
			<TimingCell
				label="Out Delay progress"
				value={formatCueSeconds(cue.out_delay_millis ?? cue.delay_millis)}
				progress={progress?.outDelay}
				onActivate={activate("outDelay")}
			/>
			<TimingCell
				label="Out Fade progress"
				value={formatCueSeconds(cue.out_fade_millis ?? cue.fade_millis)}
				progress={progress?.outFade}
				onActivate={activate("outFade")}
			/>
		</>
	);
}

function CuePreviewCell({
	cueNumber,
	thumbnail,
	onOpenPreview,
}: {
	cueNumber: number;
	thumbnail: string | undefined;
	onOpenPreview: () => void;
}) {
	return (
		<td className="cue-preview-column">
			{thumbnail && (
				<Button
					type="button"
					className="cue-preview-image-button"
					aria-label={`Open Cue ${cueNumber} preview`}
					onClick={(event) => {
						event.stopPropagation();
						onOpenPreview();
					}}
				>
					<img src={thumbnail} alt="" />
				</Button>
			)}
		</td>
	);
}

function CueTableRow({
	cue,
	index,
	cues,
	active,
	selected,
	disabled,
	propertyEditable,
	compactRows,
	thumbnail,
	mutationTarget,
	timingProgress,
	onActivateCue,
	onActivateProperty,
	onOpenPreview,
}: {
	cue: Cue;
	index: number;
	cues: Cue[];
	active: PlaybackSnapshot["active"][number] | undefined;
	selected: boolean;
	disabled: boolean;
	propertyEditable: boolean;
	compactRows: boolean;
	thumbnail: string | undefined;
	mutationTarget: ReturnType<typeof cueMutationTarget>;
	timingProgress: Partial<Record<CueTimingProgressField, number>> | undefined;
	onActivateCue: () => void;
	onActivateProperty: (property: CueEditableProperty) => void;
	onOpenPreview: () => void;
}) {
	return (
		<tr
			tabIndex={disabled ? undefined : 0}
			aria-disabled={disabled}
			onClick={() => !disabled && onActivateCue()}
			onKeyDown={(event) => {
				if (!disabled && (event.key === "Enter" || event.key === " ")) {
					event.preventDefault();
					onActivateCue();
				}
			}}
			className={`${active?.cue_index === index ? "current" : active?.effective_next_cue_number === cue.number ? "next" : ""} ${selected ? "selected" : ""} ${mutationTarget ? `${mutationTarget.operation}-target cue-command-target` : ""}`}
		>
			{!compactRows && (
				<CuePreviewCell
					cueNumber={cue.number}
					thumbnail={thumbnail}
					onOpenPreview={onOpenPreview}
				/>
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
			<td className="cue-name-column">
				<Button
					type="button"
					aria-label="Cue Name"
					disabled={!propertyEditable}
					onClick={(event) => {
						event.stopPropagation();
						onActivateProperty("name");
					}}
				>
					{cue.name || `Cue ${cue.number}`}
				</Button>
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
			<td className="cue-information-column">
				<Button
					type="button"
					aria-label="Cue Information"
					disabled={!propertyEditable}
					title={cue.information || "No Cue information"}
					onClick={(event) => {
						event.stopPropagation();
						onActivateProperty("information");
					}}
				>
					{cue.information || "—"}
				</Button>
			</td>
			<td className="cue-jump-column">
				<Button
					type="button"
					aria-label="Jump"
					disabled={!propertyEditable}
					onClick={(event) => {
						event.stopPropagation();
						onActivateProperty("jump");
					}}
				>
					{cueJumpLabel(cues, cue)}
				</Button>
			</td>
			<td className="cue-jump-count-column">
				<Button
					type="button"
					aria-label="Jump Count"
					disabled={!propertyEditable || !cueJump(cue)}
					onClick={(event) => {
						event.stopPropagation();
						onActivateProperty("jumpCount");
					}}
				>
					{cueJump(cue)?.count ?? "—"}
				</Button>
			</td>
			<td className="cue-trigger-column">
				<Button
					type="button"
					aria-label="Trigger"
					disabled={!propertyEditable}
					onClick={(event) => {
						event.stopPropagation();
						onActivateProperty("trigger");
					}}
				>
					{cueTriggerLabel(cues, cue)}
				</Button>
			</td>
			<CueTimingCells
				cue={cue}
				progress={timingProgress}
				editable={propertyEditable}
				onActivate={onActivateProperty}
			/>
		</tr>
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
	onOpenCuePreview,
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
	onOpenCuePreview?: (index: number) => void;
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
								<CueTableRow
									key={cue.number}
									cue={cue}
									index={index}
									cues={cues}
									active={active}
									selected={interactive && selectedCue === index}
									disabled={!interactive || settingsOpen}
									propertyEditable={
										interactive && !settingsOpen && Boolean(onEditCueProperty)
									}
									compactRows={compactRows}
									thumbnail={thumbnails[index]}
									mutationTarget={mutationTarget}
									timingProgress={timingProgressByRow[index]}
									onActivateCue={() => activateCue(index)}
									onActivateProperty={(property) =>
										activateProperty(index, property)
									}
									onOpenPreview={() => onOpenCuePreview?.(index)}
								/>
							))}
						</tbody>
					</table>
				)}
			</WindowScrollArea>
		</div>
	);
}
