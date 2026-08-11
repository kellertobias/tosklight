import { Button, ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import type { ControlActionSemantic } from "../../api/types";
import { useSessionSnapshot } from "../../features/deskSnapshot/DeskSnapshotState";
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
import {
	type SystemControlsTab,
	useRequestedSystemControlsTab,
} from "../../features/deskState/deskStateDiagnostics";
import { useDeskStateDiagnostics } from "../../features/deskState/DeskStateDiagnosticsState";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";
import { OutputControls } from "./systemControls/OutputControls";
import { ProgrammerList } from "./systemControls/ProgrammerList";
import { RunningSections } from "./systemControls/RunningSections";
import { useRunningDynamicsAuthority } from "./systemControls/runningDynamicsAuthority";
import { useRunningPlaybackAuthority } from "./systemControls/runningPlaybackAuthority";
import { useVisualizerViewControls } from "./systemControls/useVisualizerViewControls";
import { VisualizerControls } from "./systemControls/VisualizerControls";
import { DeskStatePanel } from "./systemControls/DeskStatePanel";

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
		)
			return;
		setStoppingAll(true);
		try {
			await Promise.all([
				...runningSources.map((source) => playbackAuthority.release(source)),
				...dynamicsAuthority.rows.map((dynamic) =>
					dynamicsAuthority.off(dynamic),
				),
				...(preload.active ? [preload.actions.release()] : []),
			]);
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

function SystemControlsTitleActions({
	model,
}: {
	model: ReturnType<typeof useSystemControlsModel>;
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
	return (
		<Button
			variant="danger"
			className="system-controls-all-off"
			disabled={cannotStop}
			onClick={() => void model.stopAllRunning()}
		>
			{model.stoppingAll ? "Turning off…" : "All Off"}
		</Button>
	);
}

export function SystemControlsModal() {
	const model = useSystemControlsModel();
	const visualizer = useVisualizerViewControls(model.open);
	const requestedTab = useRequestedSystemControlsTab();
	const deskDiagnostics = useDeskStateDiagnostics();
	const [tab, setTab] = useState<SystemControlsTab>(requestedTab);
	useEffect(() => {
		if (model.open) setTab(requestedTab);
	}, [model.open, requestedTab]);
	if (!model.open) return null;
	const activeItems =
		model.runningSources.length +
		model.dynamicsAuthority.rows.length +
		(model.preload.active ? 1 : 0);
	return (
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
						tabs={[
							{ id: "running", label: "Running" },
							{
								id: "desk-state",
								label: deskDiagnostics.length
									? `Desk State · ${deskDiagnostics.length}`
									: "Desk State",
							},
							{ id: "active-programmers", label: "Active Programmers" },
						]}
						activeTab={tab}
						onTabChange={(next) => setTab(next as SystemControlsTab)}
						details={
							<span className="system-controls-active-items">
								<b>{activeItems}</b> active items
							</span>
						}
						actions={
							tab === "running" ? (
								<SystemControlsTitleActions model={model} />
							) : null
						}
						closeLabel="Close Running & Output"
						onClose={model.close}
					/>
					{tab === "desk-state" ? (
						<DeskStatePanel diagnostics={deskDiagnostics} />
					) : tab === "active-programmers" ? (
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
					) : (
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
							{visualizer.available && (
								<VisualizerControls
									view={visualizer.view}
									targets={visualizer.targets}
									target={visualizer.target}
									busy={visualizer.busy}
									error={visualizer.error}
									onSelectTarget={visualizer.selectTarget}
									onSelectMode={visualizer.selectMode}
									onSelectQuality={visualizer.selectQuality}
								/>
							)}
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
								onReleasePreload={() =>
									void model.preload.actions?.release()
								}
								onTurnOffDynamic={(dynamic) =>
									void model.dynamicsAuthority.off(dynamic)
								}
							/>
						</div>
					)}
				</section>
			</div>
		</ModalPortal>
	);
}
