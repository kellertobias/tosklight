import {
	Button,
	ModalPortal,
	ModalRegistration,
	ModalTitleBar,
} from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import type { ControlActionSemantic } from "../../api/types";
import { useSessionSnapshot } from "../../features/deskSnapshot/DeskSnapshotState";
import { useDeskStateDiagnostics } from "../../features/deskState/DeskStateDiagnosticsState";
import {
	type SystemControlsTab,
	useRequestedSystemControlsTab,
} from "../../features/deskState/deskStateDiagnostics";
import { useDynamicsActions } from "../../features/dynamics/DynamicsActionsContext";
import {
	useOutputRuntimeActions,
	useOutputRuntimeView,
} from "../../features/outputRuntime/OutputRuntimeView";
import {
	usePatchedFixturesView,
	useSelectedPatchedFixtures,
} from "../../features/patch/PatchState";
import { useProgrammerActions } from "../../features/programmerActions/ProgrammerActionsContext";
import { useProgrammerLifecycleView } from "../../features/programmerLifecycle/ProgrammerLifecycleView";
import { useProgrammerPreloadLifecycleView } from "../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";
import { DeskStatePanel } from "./systemControls/DeskStatePanel";
import { OutputControls } from "./systemControls/OutputControls";
import { ProgrammerList } from "./systemControls/ProgrammerList";
import { RunningSections } from "./systemControls/RunningSections";
import { useRunningDynamicsAuthority } from "./systemControls/runningDynamicsAuthority";
import { useRunningPlaybackAuthority } from "./systemControls/runningPlaybackAuthority";
import { useVisualizerViewControls } from "./systemControls/useVisualizerViewControls";
import { VisualizerControls } from "./systemControls/VisualizerControls";

const EMPTY_FIXTURE_IDS: readonly string[] = [];
const EMPTY_PROGRAMMERS = [] as const;

type FixtureActions = Map<
	ControlActionSemantic,
	ReturnType<typeof compatibleSpecialDialogActions>
>;

function useFixtureActionTrigger(
	fixtureActions: FixtureActions,
	programmerActions: ReturnType<typeof useProgrammerActions>,
) {
	const [result, setResult] = useState("");
	const [latched, setLatched] = useState<Set<string>>(() => new Set());
	const trigger = async (
		semantic: ControlActionSemantic,
		phase: "click" | "press" | "release",
	) => {
		const compatible = fixtureActions.get(semantic) ?? [];
		const actions = compatible.filter((action) =>
			phase === "click"
				? action.kind !== "momentary"
				: action.kind === "momentary",
		);
		const nextLatched = new Set(latched);
		await Promise.all(
			actions.map((action) => {
				const key = `${action.fixtureId}:${action.actionId}`;
				const active =
					action.kind === "latched" ? !latched.has(key) : phase !== "release";
				if (action.kind === "latched") {
					if (active) nextLatched.add(key);
					else nextLatched.delete(key);
				}
				return programmerActions?.controlFixtureAction(
					action.fixtureId,
					action.actionId,
					active,
				);
			}),
		);
		if (actions.some((action) => action.kind === "latched"))
			setLatched(nextLatched);
		const supported = new Set(compatible.map((item) => item.fixtureId));
		const actionName = semantic
			.split("_")
			.map((part) => `${part[0]?.toUpperCase() ?? ""}${part.slice(1)}`)
			.join(" ");
		setResult(
			`${actionName} sent to ${supported.size} fixture${supported.size === 1 ? "" : "s"}`,
		);
	};
	return { result, trigger };
}

function useSystemControlsModel() {
	const { state, dispatch } = useApp();
	const programmerActions = useProgrammerActions();
	const dynamicsActions = useDynamicsActions();
	const session = useSessionSnapshot();
	const [stoppingAll, setStoppingAll] = useState(false);
	const [stopAllError, setStopAllError] = useState<string | null>(null);
	const output = useOutputRuntimeView(state.systemControlsOpen);
	const outputActions = useOutputRuntimeActions(state.systemControlsOpen);
	const selection = useProgrammingSelectionView(state.systemControlsOpen);
	const lifecycle = useProgrammerLifecycleView(state.systemControlsOpen);
	const preload = useProgrammerPreloadLifecycleView(state.systemControlsOpen);
	const playbackAuthority = useRunningPlaybackAuthority(
		state.systemControlsOpen,
	);
	const dynamicsAuthority = useRunningDynamicsAuthority(
		state.systemControlsOpen,
		output.projection?.showId ?? null,
		dynamicsActions?.dynamics ?? null,
		dynamicsActions?.events ?? null,
	);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	const fixturesSelected = selectedFixtureIds.length > 0;
	const outputReady = output.ready && outputActions !== null;
	const master = output.projection
		? Math.round(output.projection.grandMaster * 100)
		: null;
	const blackout = output.projection?.blackout ?? false;
	const selectedFixtures = useSelectedPatchedFixtures(
		selectedFixtureIds,
		state.systemControlsOpen,
	);
	const allFixtures = usePatchedFixturesView(
		state.systemControlsOpen && !fixturesSelected,
	);
	const targetFixtures = fixturesSelected ? selectedFixtures : allFixtures;
	const fixtureActions = useMemo(() => {
		const actions = new Map<
			ControlActionSemantic,
			ReturnType<typeof compatibleSpecialDialogActions>
		>();
		for (const semantic of [
			"lamp_on",
			"lamp_off",
			"reset",
			"fan_auto",
			"fan_low",
			"fan_high",
			"fan_max",
		] as const) {
			actions.set(
				semantic,
				compatibleSpecialDialogActions(
					targetFixtures,
					semantic,
					selectedFixtureIds,
				),
			);
		}
		return actions;
	}, [selectedFixtureIds, targetFixtures]);
	const programmers = lifecycle?.programmers ?? EMPTY_PROGRAMMERS;
	const runningSources = useMemo(
		() => [
			...new Map(
				playbackAuthority.sources.map((source) => [source.key, source]),
			).values(),
		],
		[playbackAuthority.sources],
	);
	const { result: fixtureActionResult, trigger: triggerFixtureAction } =
		useFixtureActionTrigger(fixtureActions, programmerActions);
	const stopAllRunning = async () => {
		if (
			!playbackAuthority.ready ||
			!dynamicsAuthority.ready ||
			!preload.ready ||
			!preload.actions ||
			(playbackAuthority.sources.length > 0 && !playbackAuthority.canRelease) ||
			(dynamicsAuthority.rows.length > 0 && !dynamicsAuthority.canStop)
		) {
			setStopAllError(
				"Running state changed before confirmation. Review the remaining items and try again.",
			);
			return false;
		}
		setStoppingAll(true);
		setStopAllError(null);
		try {
			const outcomes = await Promise.allSettled([
				...runningSources.map((source) => playbackAuthority.release(source)),
				...dynamicsAuthority.rows.map((dynamic) =>
					dynamicsAuthority.off(dynamic),
				),
				...(preload.active ? [preload.actions.release()] : []),
			]);
			const failed = outcomes.filter(
				(outcome) => outcome.status === "rejected" || outcome.value === false,
			);
			if (failed.length) {
				setStopAllError(
					`${failed.length} running item${failed.length === 1 ? "" : "s"} could not be turned off. Review the remaining items and try again.`,
				);
				return false;
			}
			return true;
		} finally {
			setStoppingAll(false);
		}
	};
	return {
		open: state.systemControlsOpen,
		programmerActions,
		session,
		master,
		blackout,
		outputReady,
		fixtureActionResult,
		stoppingAll,
		stopAllError,
		selectedFixtureIds,
		fixturesSelected,
		availableFixtureActions: new Set(
			[...fixtureActions.entries()]
				.filter(([, actions]) => actions.length > 0)
				.map(([semantic]) => semantic),
		),
		lifecycle,
		programmers,
		runningSources,
		playbackAuthority,
		dynamicsAuthority,
		preload,
		close: () =>
			dispatch({
				type: "SET_MODAL",
				modal: "systemControlsOpen",
				value: false,
			}),
		stopAllRunning,
		clearStopAllError: () => setStopAllError(null),
		triggerFixtureAction,
		setMaster: (value: number) => {
			if (!outputReady || !outputActions) return;
			void outputActions.setOutput({ grandMaster: value / 100 });
		},
		toggleBlackout: () => {
			if (!outputReady || !outputActions) return;
			const next = !blackout;
			void outputActions.setOutput({ blackout: next });
		},
	};
}

function systemControlsTitleAction({
	model,
	onRequestAllOff,
}: {
	model: ReturnType<typeof useSystemControlsModel>;
	onRequestAllOff: () => void;
}) {
	const nothingRunning =
		!model.playbackAuthority.sources.length &&
		!model.dynamicsAuthority.rows.length &&
		!model.preload.active;
	const cannotStop =
		model.stoppingAll ||
		!model.playbackAuthority.ready ||
		!model.dynamicsAuthority.ready ||
		!model.preload.ready ||
		(model.playbackAuthority.sources.length > 0 &&
			!model.playbackAuthority.canRelease) ||
		(model.dynamicsAuthority.rows.length > 0 &&
			!model.dynamicsAuthority.canStop) ||
		nothingRunning;
	return {
		id: "all-off",
		label: model.stoppingAll ? "Turning off…" : "All Off",
		variant: "danger" as const,
		className: "system-controls-all-off",
		disabled: cannotStop,
		onPress: onRequestAllOff,
	};
}

function AllOffConfirmation({
	stopping,
	error,
	onCancel,
	onConfirm,
}: {
	stopping: boolean;
	error: string | null;
	onCancel: () => void;
	onConfirm: () => void;
}) {
	return (
		<ModalRegistration onClose={onCancel}>
			<div className="stacked-modal-layer fixture-confirm-layer">
				<section
					className="nested-modal fixture-confirm-dialog"
					role="alertdialog"
					aria-modal="true"
					aria-label="Confirm All Off"
				>
					<ModalTitleBar
						title="Turn all running output off?"
						closeLabel="Cancel All Off"
						onClose={onCancel}
					/>
					<p>
						This releases every running playback, dynamic, and Programmer
						Preload. Active Programmers are not cleared.
					</p>
					{error && <p role="alert">{error}</p>}
					<div className="modal-actions">
						<Button autoFocus disabled={stopping} onClick={onCancel}>
							Cancel
						</Button>
						<Button variant="danger" disabled={stopping} onClick={onConfirm}>
							{stopping ? "Turning off…" : "Confirm All Off"}
						</Button>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}

type RunningOutputTab = SystemControlsTab | "visualizer";

function SystemControlsContent({
	activeTab,
	model,
	visualizer,
	deskDiagnostics,
}: {
	activeTab: RunningOutputTab;
	model: ReturnType<typeof useSystemControlsModel>;
	visualizer: ReturnType<typeof useVisualizerViewControls>;
	deskDiagnostics: ReturnType<typeof useDeskStateDiagnostics>;
}) {
	if (activeTab === "visualizer")
		return (
			<VisualizerControls
				view={visualizer.view}
				targets={visualizer.targets}
				target={visualizer.target}
				busy={visualizer.busy}
				error={visualizer.error}
				onSelectTarget={visualizer.selectTarget}
				onSelectMode={visualizer.selectMode}
				onSelectQuality={visualizer.selectQuality}
				onResetPhysics={visualizer.resetPhysics}
			/>
		);
	if (activeTab === "desk-state")
		return <DeskStatePanel diagnostics={deskDiagnostics} />;
	if (activeTab === "active-programmers")
		return (
			<section
				className="system-controls-programmers"
				aria-label="Active Programmers"
			>
				<ProgrammerList
					programmers={model.programmers}
					loading={model.lifecycle === null}
					currentUserId={model.session?.user.id ?? null}
					currentUserName={model.session?.user.name ?? null}
					onClear={(sessionId) =>
						void model.programmerActions?.clearProgrammer(sessionId)
					}
				/>
			</section>
		);
	return (
		<div className="system-controls-body">
			<OutputControls
				master={model.master}
				blackout={model.blackout}
				ready={model.outputReady}
				fixtureActionResult={model.fixtureActionResult}
				fixturesSelected={model.fixturesSelected}
				availableFixtureActions={model.availableFixtureActions}
				onMaster={model.setMaster}
				onBlackout={model.toggleBlackout}
				onFixtureAction={(semantic, phase) =>
					void model.triggerFixtureAction(semantic, phase)
				}
			/>
			<RunningSections
				playbacks={model.runningSources}
				dynamics={model.dynamicsAuthority.rows}
				dynamicsLoading={model.dynamicsAuthority.loading}
				dynamicsError={model.dynamicsAuthority.error}
				dynamicsCanStop={model.dynamicsAuthority.canStop}
				stoppingDynamicControllerIds={
					model.dynamicsAuthority.stoppingControllerIds
				}
				preloadActive={model.preload.active}
				playbacksLoading={model.playbackAuthority.loading}
				releaseAvailable={model.playbackAuthority.canRelease}
				onReleasePlayback={(source) =>
					void model.playbackAuthority.release(source)
				}
				onReleasePreload={() => void model.preload.actions?.release()}
				onTurnOffDynamic={(dynamic) =>
					void model.dynamicsAuthority.off(dynamic)
				}
			/>
		</div>
	);
}

export function SystemControlsModal() {
	const model = useSystemControlsModel();
	const visualizer = useVisualizerViewControls(model.open);
	const requestedTab = useRequestedSystemControlsTab();
	const deskDiagnostics = useDeskStateDiagnostics();
	const [tab, setTab] = useState<RunningOutputTab>(requestedTab);
	const [allOffConfirmationOpen, setAllOffConfirmationOpen] = useState(false);
	useEffect(() => {
		if (model.open) setTab(requestedTab);
	}, [model.open, requestedTab]);
	useEffect(() => {
		if (!visualizer.connected && tab === "visualizer") setTab("running");
	}, [tab, visualizer.connected]);
	useEffect(() => {
		if (!model.open) setAllOffConfirmationOpen(false);
	}, [model.open]);
	if (!model.open) return null;
	const activeTab =
		!visualizer.connected && tab === "visualizer" ? "running" : tab;
	const activeItems =
		model.runningSources.length +
		model.dynamicsAuthority.rows.length +
		(model.preload.active ? 1 : 0);
	const confirmAllOff = async () => {
		if (await model.stopAllRunning()) setAllOffConfirmationOpen(false);
	};
	return (
		<>
			<ModalPortal onClose={model.close}>
				<div
					className="modal-backdrop"
					onPointerDown={(event) => {
						if (event.target === event.currentTarget) model.close();
					}}
				>
					<section
						className="modal-card system-controls-card"
						role="dialog"
						aria-modal="true"
						aria-label="Running and output"
					>
						<ModalTitleBar
							className="system-controls-titlebar"
							title="Running & Output"
							details={
								<span className="system-controls-active-items">
									<b>{activeItems}</b> active items
								</span>
							}
							groups={[
								{
									id: "views",
									kind: "tabs",
									activeId: activeTab,
									onActiveChange: (next) => setTab(next as RunningOutputTab),
									actions: [
										{ id: "running", label: "Running" },
										{
											id: "desk-state",
											label: deskDiagnostics.length
												? `Desk State · ${deskDiagnostics.length}`
												: "Desk State",
										},
										{ id: "active-programmers", label: "Active Programmers" },
										...(visualizer.connected
											? [{ id: "visualizer", label: "Visualizer" }]
											: []),
									],
								},
								{
									id: "output",
									actions: [
										systemControlsTitleAction({
											model,
											onRequestAllOff: () => {
												model.clearStopAllError();
												setAllOffConfirmationOpen(true);
											},
										}),
									],
								},
							]}
							closeLabel="Close Running & Output"
							onClose={model.close}
						/>
						<SystemControlsContent
							activeTab={activeTab}
							model={model}
							visualizer={visualizer}
							deskDiagnostics={deskDiagnostics}
						/>
					</section>
				</div>
			</ModalPortal>
			{allOffConfirmationOpen && (
				<AllOffConfirmation
					stopping={model.stoppingAll}
					error={model.stopAllError}
					onCancel={() => setAllOffConfirmationOpen(false)}
					onConfirm={() => void confirmAllOff()}
				/>
			)}
		</>
	);
}
