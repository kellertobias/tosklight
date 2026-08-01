import { Button } from "@tosklight/ui";
import type { RunningCueListSource } from "./runningPlaybackAuthority";
import type { RunningDynamicController } from "./runningDynamicsAuthority";

interface RunningSectionsProps {
	playbacks: readonly RunningCueListSource[];
	dynamics: readonly RunningDynamicController[];
	dynamicsLoading: boolean;
	dynamicsError: string | null;
	dynamicsCanStop: boolean;
	stoppingDynamicControllerIds: ReadonlySet<string>;
	preloadActive: boolean;
	playbacksLoading: boolean;
	releaseAvailable: boolean;
	onReleasePlayback(source: RunningCueListSource): void;
	onReleasePreload(): void;
	onTurnOffDynamic(dynamic: RunningDynamicController): void;
}

export function RunningSections(props: RunningSectionsProps) {
	return (
		<div className="running-sections">
			<PlaybackSection
				title="Running Playbacks"
				empty="No playbacks are running."
				playbacks={props.playbacks}
				preloadActive={props.preloadActive}
				loading={props.playbacksLoading}
				releaseAvailable={props.releaseAvailable}
				onRelease={props.onReleasePlayback}
				onReleasePreload={props.onReleasePreload}
			/>
			<DynamicsSection
				dynamics={props.dynamics}
				loading={props.dynamicsLoading}
				error={props.dynamicsError}
				canStop={props.dynamicsCanStop}
				stoppingControllerIds={props.stoppingDynamicControllerIds}
				onTurnOff={props.onTurnOffDynamic}
			/>
		</div>
	);
}

interface PlaybackSectionProps {
	title: string;
	empty: string;
	playbacks: readonly RunningCueListSource[];
	preloadActive: boolean;
	loading: boolean;
	releaseAvailable: boolean;
	onRelease(source: RunningCueListSource): void;
	onReleasePreload(): void;
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
						releaseAvailable={props.releaseAvailable}
						onRelease={props.onRelease}
					/>
				))}
				{props.preloadActive && (
					<article>
						<span>
							<b>Preload</b>
							<small>Programmer Preload · Running</small>
						</span>
						<Button
							className="danger"
							aria-label="Turn off Preload"
							onClick={props.onReleasePreload}
						>
							Off
						</Button>
					</article>
				)}
				{!props.playbacks.length && !props.preloadActive && (
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
	releaseAvailable,
	onRelease,
}: {
	playback: RunningCueListSource;
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
						? "Virtual playback"
						: `Playback ${playback.playbackNumber}`} {" "}
					· Cue {cueNumber} · {Math.round(playback.runtime.master * 100)}% ·{" "}
					{playback.runtime.paused ? "Paused" : "Running"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Turn off ${
					playback.playbackNumber == null ? "Virtual playback" : "Playback"
				} ${playback.label}`}
				disabled={!releaseAvailable}
				onClick={() => onRelease(playback)}
			>
				Off
			</Button>
		</article>
	);
}

function DynamicsSection({
	dynamics,
	loading,
	error,
	canStop,
	stoppingControllerIds,
	onTurnOff,
}: {
	dynamics: readonly RunningDynamicController[];
	loading: boolean;
	error: string | null;
	canStop: boolean;
	stoppingControllerIds: ReadonlySet<string>;
	onTurnOff(dynamic: RunningDynamicController): void;
}) {
	return (
		<section>
			<h3>
				Running Dynamics <small>{dynamics.length}</small>
			</h3>
			<div className="programmer-list">
				{dynamics.map((dynamic) => (
					<DynamicRow
						key={dynamic.key}
						dynamic={dynamic}
						canStop={canStop}
						stopping={stoppingControllerIds.has(dynamic.controllerId)}
						onTurnOff={onTurnOff}
					/>
				))}
				{!dynamics.length && (
					<p className="empty-window-message">
						{loading
							? "Dynamics loading…"
							: error
								? `Dynamics unavailable: ${error}`
								: "No dynamics are running."}
					</p>
				)}
				{dynamics.length > 0 && error && (
					<p className="modal-status" role="alert">
						{error}
					</p>
				)}
			</div>
		</section>
	);
}

function DynamicRow({
	dynamic,
	canStop,
	stopping,
	onTurnOff,
}: {
	dynamic: RunningDynamicController;
	canStop: boolean;
	stopping: boolean;
	onTurnOff(dynamic: RunningDynamicController): void;
}) {
	return (
		<article>
			<span>
				<b>
					{dynamic.name} · Dynamic {dynamic.poolNumber}
				</b>
				<small>
					{dynamic.source} · {dynamic.targets.length} target
					{dynamic.targets.length === 1 ? "" : "s"} ·{" "}
					{dynamic.paused || dynamic.instancePaused ? "Paused" : "Running"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Turn off Dynamic ${dynamic.poolNumber} ${dynamic.name} from ${dynamic.source}`}
				disabled={!canStop || stopping || dynamic.releasing}
				onClick={() => onTurnOff(dynamic)}
			>
				{stopping ? "Turning off…" : "Off"}
			</Button>
		</article>
	);
}
