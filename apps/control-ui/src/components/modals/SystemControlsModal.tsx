import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useProgrammerLifecycleView } from "../../features/programmerLifecycle/ProgrammerLifecycleView";
import { useProgrammerPreloadLifecycleView } from "../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import { Button, ModalPortal } from "../common";
import { compatibleSpecialDialogActions } from "./SpecialDialogsModal";
import { OutputControls } from "./systemControls/OutputControls";
import { RunningSections } from "./systemControls/RunningSections";
import { useRunningPlaybackAuthority } from "./systemControls/runningPlaybackAuthority";

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
	const preload = useProgrammerPreloadLifecycleView(state.systemControlsOpen);
	const playbackAuthority = useRunningPlaybackAuthority(state.systemControlsOpen);
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
	const programmers = lifecycle?.programmers ?? EMPTY_PROGRAMMERS;
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
		if (
			!playbackAuthority.ready ||
			!preload.ready ||
			!preload.actions ||
			(playbackAuthority.sources.length > 0 && !playbackAuthority.canRelease)
		)
			return;
		setStoppingAll(true);
		try {
			const sources = new Map(
				playbackAuthority.sources.map((source) => [source.key, source]),
			);
			await Promise.all([
				...[...sources.values()].map((source) =>
					playbackAuthority.release(source),
				),
				...programmers.flatMap((programmer) =>
					programmer.sessions[0]
						? [server.clearProgrammer(programmer.sessions[0].sessionId)]
						: [],
				),
				preload.actions.release(),
			]);
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
		playbackAuthority,
		preload,
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
		model.playbackAuthority.mappedSources.length +
		model.playbackAuthority.virtualSources.length +
		model.programmers.length +
		model.playbackAuthority.dynamics.length +
		(model.preload.active ? 1 : 0);
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
								!model.playbackAuthority.ready ||
								!model.preload.ready ||
								(model.playbackAuthority.sources.length > 0 &&
									!model.playbackAuthority.canRelease) ||
								(!model.playbackAuthority.sources.length &&
									!model.programmers.length &&
									!model.preload.active)
							}
							onClick={() => void model.stopEverything()}
						>
							{model.stoppingAll ? "Stopping…" : "Stop everything"}
						</Button>
					</div>
					<RunningSections
						pagePlaybacks={model.playbackAuthority.mappedSources}
						virtualPlaybacks={model.playbackAuthority.virtualSources}
						dynamics={model.playbackAuthority.dynamics}
						playbacksLoading={model.playbackAuthority.loading}
						releaseAvailable={model.playbackAuthority.canRelease}
						programmers={model.programmers}
						programmersLoading={model.lifecycle === null}
						currentUserId={model.server.session?.user.id ?? null}
						currentUserName={model.server.session?.user.name ?? null}
						onReleasePlayback={(source) =>
							void model.playbackAuthority.release(source)
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
