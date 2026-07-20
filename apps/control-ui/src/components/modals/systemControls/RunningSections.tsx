import type { PlaybackSnapshot } from "../../../api/types";
import type { ProgrammerLifecycleRow } from "../../../features/programmerLifecycle/contracts";
import { Button } from "../../common";
import { ProgrammerList } from "./ProgrammerList";

type ActivePlayback = PlaybackSnapshot["active"][number];
type CueList = PlaybackSnapshot["cue_lists"][number];
type Cue = CueList["cues"][number];

export interface RunningDynamic {
	playback: ActivePlayback;
	cueList: CueList | undefined;
	cue: Cue | undefined;
	index: number;
}

interface RunningSectionsProps {
	playbacks: PlaybackSnapshot | null;
	pagePlaybacks: readonly ActivePlayback[];
	virtualPlaybacks: readonly ActivePlayback[];
	dynamics: readonly RunningDynamic[];
	programmers: readonly ProgrammerLifecycleRow[];
	programmersLoading: boolean;
	currentUserId: string | null;
	currentUserName: string | null;
	onReleasePlayback(cueListId: string): void;
	onClearProgrammer(sessionId: string): void;
}

export function RunningSections(props: RunningSectionsProps) {
	return (
		<div className="running-sections">
			<PlaybackSection
				title="Virtual playbacks"
				empty="No virtual playbacks are running."
				source="Virtual playback"
				playbacks={props.virtualPlaybacks}
				snapshot={props.playbacks}
				onRelease={props.onReleasePlayback}
			/>
			<PlaybackSection
				title="Playbacks"
				empty="No playbacks are running."
				source="Playback"
				playbacks={props.pagePlaybacks}
				snapshot={props.playbacks}
				onRelease={props.onReleasePlayback}
			/>
			<ProgrammerList
				programmers={props.programmers}
				loading={props.programmersLoading}
				currentUserId={props.currentUserId}
				currentUserName={props.currentUserName}
				onClear={props.onClearProgrammer}
			/>
			<DynamicsSection
				dynamics={props.dynamics}
				onRelease={props.onReleasePlayback}
			/>
		</div>
	);
}

function PlaybackSection({
	title,
	empty,
	source,
	playbacks,
	snapshot,
	onRelease,
}: {
	title: string;
	empty: string;
	source: "Playback" | "Virtual playback";
	playbacks: readonly ActivePlayback[];
	snapshot: PlaybackSnapshot | null;
	onRelease(cueListId: string): void;
}) {
	return (
		<section>
			<h3>
				{title} <small>{playbacks.length}</small>
			</h3>
			<div className="programmer-list">
				{playbacks.map((playback) => (
					<PlaybackRow
						key={playback.cue_list_id}
						playback={playback}
						snapshot={snapshot}
						source={source}
						onRelease={onRelease}
					/>
				))}
				{!playbacks.length && <p className="empty-window-message">{empty}</p>}
			</div>
		</section>
	);
}

function PlaybackRow({
	playback,
	snapshot,
	source,
	onRelease,
}: {
	playback: ActivePlayback;
	snapshot: PlaybackSnapshot | null;
	source: "Playback" | "Virtual playback";
	onRelease(cueListId: string): void;
}) {
	const cueList = snapshot?.cue_lists.find(
		(candidate) => candidate.id === playback.cue_list_id,
	);
	const cue = cueList?.cues[playback.cue_index];
	const definition =
		playback.playback_number == null
			? null
			: snapshot?.pool.find(
					(candidate) => candidate.number === playback.playback_number,
				);
	const label =
		definition?.name ||
		cueList?.name ||
		`Cuelist ${playback.cue_list_id.slice(0, 8)}`;
	return (
		<article>
			<span>
				<b>{label}</b>
				<small>
					{playback.playback_number == null
						? source
						: `Playback ${playback.playback_number}`}{" "}
					· Cue {cue?.number ?? playback.cue_index + 1} ·{" "}
					{Math.round(playback.master * 100)}% ·{" "}
					{playback.paused ? "Paused" : "Running"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Stop ${source} ${label}`}
				onClick={() => onRelease(playback.cue_list_id)}
			>
				Stop
			</Button>
		</article>
	);
}

function DynamicsSection({
	dynamics,
	onRelease,
}: {
	dynamics: readonly RunningDynamic[];
	onRelease(cueListId: string): void;
}) {
	return (
		<section>
			<h3>
				Dynamics <small>{dynamics.length}</small>
			</h3>
			<div className="programmer-list">
				{dynamics.map((dynamic) => (
					<DynamicRow
						key={`${dynamic.playback.cue_list_id}-${dynamic.index}`}
						dynamic={dynamic}
						onRelease={onRelease}
					/>
				))}
				{!dynamics.length && (
					<p className="empty-window-message">No dynamics are running.</p>
				)}
			</div>
		</section>
	);
}

function DynamicRow({
	dynamic,
	onRelease,
}: {
	dynamic: RunningDynamic;
	onRelease(cueListId: string): void;
}) {
	const { playback, cueList, cue, index } = dynamic;
	const source = cueList?.name ?? "Cuelist";
	return (
		<article>
			<span>
				<b>
					{source} · Dynamic {index + 1}
				</b>
				<small>
					Cue {cue?.number ?? playback.cue_index + 1} · Stop releases its source
					playback
				</small>
			</span>
			<Button
				className="danger"
				title="Stops this Dynamic by releasing its source playback"
				aria-label={`Stop Dynamic ${index + 1} from ${source}`}
				onClick={() => onRelease(playback.cue_list_id)}
			>
				Stop
			</Button>
		</article>
	);
}
