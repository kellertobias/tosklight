import { Button, NumberField, SelectField, TextField } from "@tosklight/ui";
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
	const draft = controller.draft;
	if (!draft) return null;
	return (
		<>
			<h2>Timecode</h2>
			<div className="setup-form-grid">
				<TimecodeSourceFields controller={controller} />
				<TimecodeLossFields controller={controller} />
			</div>
		</>
	);
}

function TimecodeSourceFields({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const draft = controller.draft;
	if (!draft) return null;
	const externalSource =
		draft.timecode_source.type === "external"
			? draft.timecode_source.source
			: "";
	const frameRate = draft.timecode_frame_rate
		? draft.timecode_frame_rate.numerator /
			draft.timecode_frame_rate.denominator
		: 0;
	return (
		<>
			<TextField
				label="ArtTimeCode UDP bind"
				value={draft.art_timecode_bind ?? ""}
				description="Listen for Art-Net ArtTimeCode on this local address, for example 0.0.0.0:6454. Leave empty to disable the network source."
				onChange={(event) =>
					controller.editDraft({
						...draft,
						art_timecode_bind: event.target.value || null,
					})
				}
			/>
			<SelectField
				label="Authoritative source"
				value={draft.timecode_source.type}
				options={[
					{ value: "internal", label: "Internal generator" },
					{ value: "external", label: "Explicit external source" },
				]}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						timecode_source:
							value === "external"
								? { type: "external", source: "" }
								: { type: "internal" },
					})
				}
			/>
			{draft.timecode_source.type === "external" && (
				<TextField
					label="External source identity"
					value={externalSource}
					description="Use the exact normalized identity shown by the input adapter. No other sender can take over."
					onChange={(event) =>
						controller.editDraft({
							...draft,
							timecode_source: {
								type: "external",
								source: event.target.value,
							},
						})
					}
				/>
			)}
			<NumberField
				label="Timecode frame rate"
				value={frameRate}
				min={0}
				max={240}
				description="0 follows the desk DMX frame rate. A non-zero value converts known incoming rates with a warning."
				onChange={(event) => {
					const value = Number(event.target.value);
					controller.editDraft({
						...draft,
						timecode_frame_rate:
							value > 0
								? { numerator: value, denominator: 1, drop_frame: false }
								: null,
					});
				}}
			/>
		</>
	);
}

function TimecodeLossFields({
	controller,
}: {
	controller: SetupWindowController;
}) {
	const draft = controller.draft;
	if (!draft) return null;
	return (
		<>
			<SelectField
				label="External source loss"
				value={draft.timecode_external_loss_policy}
				options={[
					{ value: "continue_internal", label: "Continue internally" },
					{ value: "pause", label: "Pause" },
					{ value: "stop", label: "Stop" },
				]}
				onChange={(value) =>
					controller.editDraft({
						...draft,
						timecode_external_loss_policy: value as
							| "continue_internal"
							| "pause"
							| "stop",
					})
				}
			/>
			<NumberField
				label="Loss timeout"
				value={draft.timecode_external_loss_timeout_millis}
				min={1}
				max={60_000}
				unit="ms"
				onChange={(event) =>
					controller.editDraft({
						...draft,
						timecode_external_loss_timeout_millis: Number(event.target.value),
					})
				}
			/>
		</>
	);
}
