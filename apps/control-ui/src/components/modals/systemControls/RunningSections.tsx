import type { ProgrammerLifecycleRow } from "../../../features/programmerLifecycle/contracts";
import { Button } from "../../common";
import { ProgrammerList } from "./ProgrammerList";
import type {
	RunningCueListSource,
	RunningDynamic,
} from "./runningPlaybackAuthority";

interface RunningSectionsProps {
	pagePlaybacks: readonly RunningCueListSource[];
	virtualPlaybacks: readonly RunningCueListSource[];
	dynamics: readonly RunningDynamic[];
	playbacksLoading: boolean;
	releaseAvailable: boolean;
	programmers: readonly ProgrammerLifecycleRow[];
	programmersLoading: boolean;
	currentUserId: string | null;
	currentUserName: string | null;
	onReleasePlayback(source: RunningCueListSource): void;
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
				loading={props.playbacksLoading}
				releaseAvailable={props.releaseAvailable}
				onRelease={props.onReleasePlayback}
			/>
			<PlaybackSection
				title="Playbacks"
				empty="No playbacks are running."
				source="Playback"
				playbacks={props.pagePlaybacks}
				loading={props.playbacksLoading}
				releaseAvailable={props.releaseAvailable}
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
				loading={props.playbacksLoading}
				releaseAvailable={props.releaseAvailable}
				onRelease={props.onReleasePlayback}
			/>
		</div>
	);
}

interface PlaybackSectionProps {
	title: string;
	empty: string;
	source: "Playback" | "Virtual playback";
	playbacks: readonly RunningCueListSource[];
	loading: boolean;
	releaseAvailable: boolean;
	onRelease(source: RunningCueListSource): void;
}

function PlaybackSection(props: PlaybackSectionProps) {
	return (
		<section>
			<h3>
				{props.title} <small>{props.playbacks.length}</small>
			</h3>
			<div className="programmer-list">
				{props.playbacks.map((playback) => (
					<PlaybackRow
						key={playback.key}
						playback={playback}
						source={props.source}
						releaseAvailable={props.releaseAvailable}
						onRelease={props.onRelease}
					/>
				))}
				{!props.playbacks.length && (
					<p className="empty-window-message">
						{props.loading ? `${props.title} loading…` : props.empty}
					</p>
				)}
			</div>
		</section>
	);
}

function PlaybackRow({
	playback,
	source,
	releaseAvailable,
	onRelease,
}: {
	playback: RunningCueListSource;
	source: "Playback" | "Virtual playback";
	releaseAvailable: boolean;
	onRelease(source: RunningCueListSource): void;
}) {
	const cueNumber =
		playback.cue?.number ??
		playback.runtime.current?.number ??
		playback.runtime.cue_index + 1;
	return (
		<article>
			<span>
				<b>{playback.label}</b>
				<small>
					{playback.playbackNumber == null
						? source
						: `Playback ${playback.playbackNumber}`} {" "}
					· Cue {cueNumber} · {Math.round(playback.runtime.master * 100)}% ·{" "}
					{playback.runtime.paused ? "Paused" : "Running"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Stop ${source} ${playback.label}`}
				disabled={!releaseAvailable}
				onClick={() => onRelease(playback)}
			>
				Stop
			</Button>
		</article>
	);
}

function DynamicsSection({
	dynamics,
	loading,
	releaseAvailable,
	onRelease,
}: {
	dynamics: readonly RunningDynamic[];
	loading: boolean;
	releaseAvailable: boolean;
	onRelease(source: RunningCueListSource): void;
}) {
	return (
		<section>
			<h3>
				Dynamics <small>{dynamics.length}</small>
			</h3>
			<div className="programmer-list">
				{dynamics.map((dynamic) => (
					<DynamicRow
						key={`${dynamic.source.key}-${dynamic.index}`}
						dynamic={dynamic}
						releaseAvailable={releaseAvailable}
						onRelease={onRelease}
					/>
				))}
				{!dynamics.length && (
					<p className="empty-window-message">
						{loading ? "Dynamics loading…" : "No dynamics are running."}
					</p>
				)}
			</div>
		</section>
	);
}

function DynamicRow({
	dynamic,
	releaseAvailable,
	onRelease,
}: {
	dynamic: RunningDynamic;
	releaseAvailable: boolean;
	onRelease(source: RunningCueListSource): void;
}) {
	const { source, index } = dynamic;
	const sourceLabel = source.cueList?.name ?? "Cuelist";
	const cueNumber =
		source.cue?.number ??
		source.runtime.current?.number ??
		source.runtime.cue_index + 1;
	return (
		<article>
			<span>
				<b>
					{sourceLabel} · Dynamic {index + 1}
				</b>
				<small>
					Cue {cueNumber} · Stop releases its source playback
				</small>
			</span>
			<Button
				className="danger"
				title="Stops this Dynamic by releasing its source playback"
				aria-label={`Stop Dynamic ${index + 1} from ${sourceLabel}`}
				disabled={!releaseAvailable}
				onClick={() => onRelease(source)}
			>
				Stop
			</Button>
		</article>
	);
}
