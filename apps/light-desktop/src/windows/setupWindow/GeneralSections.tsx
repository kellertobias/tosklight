import { Button, NumberField } from "@tosklight/ui";
import { ShowRecoveryFileManager } from "../../components/setup/ShowRecoveryFileManager";
import { useBootstrapSnapshot } from "../../features/deskSnapshot/DeskSnapshotState";
import { useConnectionStatus } from "../../features/shellStatus/ShellStatusState";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
import { useApp } from "../../state/AppContext";
import type { SetupWindowController } from "./controller";

export function ShowsRecoverySection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const bootstrap = useBootstrapSnapshot();
	const lifecycle = useShowLifecycle();
	const connectionStatus = useConnectionStatus();
	const { dispatch } = useApp();
	const { draft } = controller;
	const activeShow = bootstrap?.active_show;
	const autosaveActive =
		connectionStatus === "connected" && Boolean(activeShow);
	const autosaveStatus = autosaveActive
		? "Connected, autosave active"
		: connectionStatus === "connected"
			? "Connected, no active show"
			: `${connectionStatus}, autosave paused`;
	return (
		<>
			<h2>Shows & recovery</h2>
			<div className="setup-show-summary">
				<section>
					<b>Current show</b>
					<span>{activeShow?.name ?? "No show loaded"}</span>
					<small>
						{activeShow?.updated_at ?? "Choose a show from the library"}
					</small>
				</section>
				<section>
					<b>Show library</b>
					<span>{lifecycle?.shows.length ?? 0} library shows</span>
					<Button
						onClick={() =>
							dispatch({ type: "OPEN_BUILTIN", kind: "file_manager" })
						}
					>
						Open show
					</Button>
				</section>
				<section>
					{draft && (
						<NumberField
							label="Autosave interval"
							min="5"
							max="3600"
							value={draft.autosave_interval_seconds}
							onChange={(event) =>
								controller.editDraft({
									...draft,
									autosave_interval_seconds: Number(event.target.value),
								})
							}
						/>
					)}
					<small className={autosaveActive ? "is-connected" : ""}>
						{autosaveStatus}
					</small>
				</section>
			</div>
			<ShowRecoveryFileManager
				onOpenFixtureLibrary={() => controller.setFixtureLibraryOpen(true)}
			/>
		</>
	);
}

export function TimecodeSection({
	controller,
}: {
	controller: SetupWindowController;
}) {
	return (
		<>
			<h2>Timecode</h2>
			<div className="setup-list">
				{controller.draft?.timecode_sources.map((source) => (
					<article key={source.source_prefix}>
						<b>{source.source_prefix}</b>
						<span>Priority {source.priority}</span>
						<small>
							{source.fallback ? "Fallback allowed" : "Explicit source only"}
						</small>
					</article>
				))}
			</div>
		</>
	);
}
