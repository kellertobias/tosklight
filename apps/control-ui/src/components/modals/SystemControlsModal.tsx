import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useProgrammerLifecycleView } from "../../features/programmerLifecycle/ProgrammerLifecycleView";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import { Button, ModalPortal } from "../common";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";
import { OutputControls } from "./systemControls/OutputControls";
import {
	type RunningDynamic,
	RunningSections,
} from "./systemControls/RunningSections";

const EMPTY_FIXTURE_IDS: readonly string[] = [];
const EMPTY_PROGRAMMERS = [] as const;

function useSystemControlsModel() {
	const { state, dispatch } = useApp();
	const server = useServer();
	const [master, setMaster] = useState(100);
	const [blackout, setBlackout] = useState(false);
	const [lampResult, setLampResult] = useState("");
	const [stoppingAll, setStoppingAll] = useState(false);
	const selection = useProgrammingSelectionView(state.systemControlsOpen);
	const lifecycle = useProgrammerLifecycleView(state.systemControlsOpen);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	useEffect(() => {
		if (!state.systemControlsOpen) return;
		void server.readVisualization().then((snapshot) => {
			setMaster(Math.round(snapshot.grand_master * 100));
			setBlackout(snapshot.blackout);
			dispatch({ type: "SET_BLACKOUT", value: snapshot.blackout });
		});
	}, [state.systemControlsOpen, server.readVisualization, dispatch]);
	const lampActions = useMemo(
		() =>
			compatibleSpecialDialogActions(
				server.patch?.fixtures ?? [],
				"lamp_on",
				selectedFixtureIds,
			),
		[server.patch, selectedFixtureIds],
	);
	const runningPlaybacks = server.playbacks?.active ?? [];
	const pagePlaybacks = runningPlaybacks.filter(
		(playback) => playback.playback_number != null,
	);
	const virtualPlaybacks = runningPlaybacks.filter(
		(playback) => playback.playback_number == null,
	);
	const programmers = lifecycle?.programmers ?? EMPTY_PROGRAMMERS;
	const dynamics: RunningDynamic[] = runningPlaybacks.flatMap((playback) => {
		const cueList = server.playbacks?.cue_lists.find(
			(candidate) => candidate.id === playback.cue_list_id,
		);
		const cue = cueList?.cues[playback.cue_index];
		return (cue?.phasers ?? []).map((_, index) => ({
			playback,
			cueList,
			cue,
			index,
		}));
	});
	const triggerLamps = async (phase: "click" | "press" | "release") => {
		const actions = lampActions.filter((action) =>
			phase === "click"
				? action.kind !== "momentary"
				: action.kind === "momentary",
		);
		await Promise.all(
			actions.map((action) =>
				server.controlFixtureAction(
					action.fixtureId,
					action.actionId,
					phase !== "release",
				),
			),
		);
		const supported = new Set(lampActions.map((item) => item.fixtureId));
		const skipped = Math.max(0, selectedFixtureIds.length - supported.size);
		setLampResult(
			`${supported.size} discharge lamp${supported.size === 1 ? "" : "s"} triggered${skipped ? ` · ${skipped} without Lamp On skipped` : ""}`,
		);
	};
	const stopEverything = async () => {
		setStoppingAll(true);
		try {
			await Promise.all([
				...runningPlaybacks.map((playback) =>
					server.playbackAction(playback.cue_list_id, "release"),
				),
				...programmers.flatMap((programmer) =>
					programmer.sessions[0]
						? [server.clearProgrammer(programmer.sessions[0].sessionId)]
						: [],
				),
				server.preloadAction("release"),
			]);
			dispatch({ type: "RELEASE_PRELOAD" });
		} finally {
			setStoppingAll(false);
		}
	};
	return {
		open: state.systemControlsOpen,
		server,
		master,
		blackout,
		lampResult,
		stoppingAll,
		selectedFixtureIds,
		lifecycle,
		programmers,
		runningPlaybacks,
		pagePlaybacks,
		virtualPlaybacks,
		dynamics,
		close: () =>
			dispatch({
				type: "SET_MODAL",
				modal: "systemControlsOpen",
				value: false,
			}),
		stopEverything,
		triggerLamps,
		setMaster: (value: number) => {
			setMaster(value);
			void server.setMaster(value / 100, undefined);
		},
		toggleBlackout: () => {
			const next = !blackout;
			setBlackout(next);
			dispatch({ type: "SET_BLACKOUT", value: next });
			void server.setMaster(undefined, next);
		},
	};
}

export function SystemControlsModal() {
	const model = useSystemControlsModel();
	if (!model.open) return null;
	const activeItems =
		model.pagePlaybacks.length +
		model.virtualPlaybacks.length +
		model.programmers.length +
		model.dynamics.length;
	return (
		<ModalPortal>
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
					<header className="system-controls-header">
						<div>
							<h2>Running & Output</h2>
							<small>Shift + Clear / Shift + Delete</small>
						</div>
						<Button className="modal-close" onClick={model.close}>
							×
						</Button>
					</header>
					<div className="running-summary">
						<span>
							<b>{activeItems}</b> active items
						</span>
						<Button
							className="danger"
							disabled={
								model.stoppingAll ||
								(!model.runningPlaybacks.length && !model.programmers.length)
							}
							onClick={() => void model.stopEverything()}
						>
							{model.stoppingAll ? "Stopping…" : "Stop everything"}
						</Button>
					</div>
					<RunningSections
						playbacks={model.server.playbacks}
						pagePlaybacks={model.pagePlaybacks}
						virtualPlaybacks={model.virtualPlaybacks}
						dynamics={model.dynamics}
						programmers={model.programmers}
						programmersLoading={model.lifecycle === null}
						currentUserId={model.server.session?.user.id ?? null}
						currentUserName={model.server.session?.user.name ?? null}
						onReleasePlayback={(cueListId) =>
							void model.server.playbackAction(cueListId, "release")
						}
						onClearProgrammer={(sessionId) =>
							void model.server.clearProgrammer(sessionId)
						}
					/>
					<OutputControls
						master={model.master}
						blackout={model.blackout}
						lampResult={model.lampResult}
						lampActionsAvailable={model.selectedFixtureIds.length > 0}
						onMaster={model.setMaster}
						onBlackout={model.toggleBlackout}
						onLamp={(phase) => void model.triggerLamps(phase)}
					/>
				</section>
			</div>
		</ModalPortal>
	);
}
