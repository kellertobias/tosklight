import type { Cue, PlaybackSnapshot } from "../../api/types";
import { WindowScrollArea } from "../../components/window-kit";
import { cueTriggerKind } from "./cueFormatting";

export interface CueTableEmptyState {
	title: string;
	description: string;
	icon: string;
}

export function CueTable({
	cues,
	active,
	selectedCue,
	settingsOpen,
	thumbnails,
	emptyState,
	onSelectCue,
}: {
	cues: Cue[];
	active: PlaybackSnapshot["active"][number] | undefined;
	selectedCue: number;
	settingsOpen: boolean;
	thumbnails: Record<number, string>;
	emptyState: CueTableEmptyState;
	onSelectCue: (index: number) => void;
}) {
	return (
		<div className="cue-editor">
			<WindowScrollArea
				className="cue-table-wrap"
				emptyState={cues.length ? null : emptyState}
			>
				{cues.length > 0 && (
					<table className="cue-table">
						<thead>
							<tr>
								<th>Preview</th>
								<th>No.</th>
								<th>Name</th>
								<th>Trigger</th>
								<th>Fade</th>
							</tr>
						</thead>
						<tbody>
							{cues.map((cue, index) => (
								<tr
									tabIndex={0}
									aria-disabled={settingsOpen}
									onClick={() => {
										if (!settingsOpen) onSelectCue(index);
									}}
									onKeyDown={(event) => {
										if (settingsOpen) return;
										if (event.key === "Enter" || event.key === " ") {
											event.preventDefault();
											onSelectCue(index);
										}
									}}
									key={cue.number}
									className={`${active?.cue_index === index ? "current" : active?.cue_index === index - 1 ? "next" : ""} ${selectedCue === index ? "selected" : ""}`}
								>
									<td>
										{thumbnails[index] && (
											<img src={thumbnails[index]} alt="" />
										)}
									</td>
									<td>
										<b>{cue.number}</b>
									</td>
									<td>{cue.name || `Cue ${cue.number}`}</td>
									<td>{cueTriggerKind(cue).toUpperCase()}</td>
									<td>
										{(cue.fade_millis / 1000).toFixed(3).replace(/\.?0+$/, "")}{" "}
										{"s"}
									</td>
								</tr>
							))}
						</tbody>
					</table>
				)}
			</WindowScrollArea>
		</div>
	);
}
