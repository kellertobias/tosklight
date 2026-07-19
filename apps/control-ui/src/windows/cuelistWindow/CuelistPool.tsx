import { useMemo, useRef, useState } from "react";
import { useServer } from "../../api/ServerContext";
import type {
	PlaybackDefinition,
	PlaybackPage,
	PlaybackSnapshot,
} from "../../api/types";
import { Button, SearchBar } from "../../components/common";
import {
	cueUpdateTarget,
	requestUpdateTarget,
} from "../../components/control/updateWorkflow";
import {
	ButtonGrid,
	WindowHeader,
	WindowScrollArea,
} from "../../components/window-kit";
import { runtimeMaster } from "../../features/playbackRuntime/legacy";
import { usePlaybackProjectionMap } from "../../features/playbackRuntime/PlaybackRuntimeView";
import { useApp } from "../../state/AppContext";

interface CuelistPoolProps {
	active: boolean;
	compact?: boolean;
	builtIn: boolean;
	selectedCuelist: number | null;
	message: string;
	onMessage: (message: string) => void;
	onOpenCuelist: (number: number) => void;
	onSelectLocalCuelist: (number: number) => void;
	onOpenSettings: (number: number) => void;
	settings: React.ReactNode;
}

interface PoolSlotProps {
	number: number;
	playback: PlaybackDefinition | null;
	selectedCuelist: number | null;
	runtimeMaster: number | null;
	usage: number[];
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: boolean;
	onPointerDown: () => void;
	onPointerEnd: () => void;
	onClick: () => void;
}

function CuelistPoolSlot(props: PoolSlotProps) {
	const { number, playback, runtimeMaster, usage } = props;
	return (
		<Button
			className={`pool-cell cuelist-card ${playback ? "" : "empty"} ${runtimeMaster != null ? "running" : ""} ${props.selectedCuelist === number && playback ? "selected" : ""} ${props.storeArmed ? "store-target" : ""} ${props.updateArmed ? "update-target" : ""} ${props.setTarget ? "set-target" : ""}`}
			onPointerDown={props.onPointerDown}
			onPointerUp={props.onPointerEnd}
			onPointerCancel={props.onPointerEnd}
			onContextMenu={(event) => event.preventDefault()}
			onClick={props.onClick}
		>
			<span className="number">{number}</span>
			<b>{playback?.name ?? "Empty"}</b>
			{playback ? (
				<>
					<small>
						{props.updateArmed
							? "Touch to choose Update mode"
							: `Cuelist · ${runtimeMaster != null ? `${Math.round(runtimeMaster * 100)}%` : "Off"}`}
					</small>
					<small>
						{usage.length
							? `Playbacks on pages ${usage.join(", ")}`
							: "Not assigned to a playback"}
					</small>
				</>
			) : (
				<small>
					{props.updateArmed
						? "Touch to check Update eligibility"
						: props.storeArmed
							? "Tap to record Cuelist"
							: "Press Rec first"}
				</small>
			)}
		</Button>
	);
}

function useCuelistPoolActions(props: CuelistPoolProps) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const holdTimer = useRef<number | null>(null);
	const held = useRef(false);
	const clearHold = () => {
		if (holdTimer.current) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	const startHold = (number: number, playback: PlaybackDefinition | null) => {
		if (!playback || state.updateArmed) return;
		held.current = false;
		holdTimer.current = window.setTimeout(() => {
			held.current = true;
			props.onOpenSettings(number);
		}, 650);
	};
	const click = (number: number, playback: PlaybackDefinition | null) => {
		if (held.current) {
			held.current = false;
			return;
		}
		if (state.updateArmed) {
			const objectId =
				playback?.target.type === "cue_list"
					? playback.target.cue_list_id
					: String(number);
			requestUpdateTarget(cueUpdateTarget(objectId));
			return;
		}
		if (state.storeArmed) {
			void server
				.executeCommandLine(`RECORD SET ${number}`)
				.then(async (ok) => {
					if (!ok) return;
					server.setCommandLine("");
					await server.refresh();
					dispatch({ type: "SET_STORE_ARMED", value: false });
				});
			return;
		}
		if (state.cueListSetArmed) {
			if (!playback) {
				props.onMessage(
					`Cuelist ${number} is empty · record it before assigning it to a playback.`,
				);
				return;
			}
			if (!props.builtIn) props.onSelectLocalCuelist(number);
			dispatch({ type: "SET_CUELIST_SET_TARGET", value: number });
			dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
			return;
		}
		if (!playback) return;
		props.onMessage("");
		props.onOpenCuelist(number);
	};
	return { server, state, clearHold, startHold, click };
}

function usePoolSlots(
	pool: PlaybackDefinition[],
	search: string,
	pages: PlaybackPage[] | undefined,
	runtimes: ReturnType<typeof usePlaybackProjectionMap>,
	legacyRuntime: PlaybackSnapshot["active"] | undefined,
) {
	return useMemo(() => {
		const byNumber = new Map(
			pool.map((playback) => [playback.number, playback]),
		);
		const usageByNumber = new Map<number, number[]>();
		const legacyMasters = new Map(
			(legacyRuntime ?? []).map((runtime) => [
				runtime.playback_number,
				runtime.master,
			]),
		);
		for (const page of pages ?? []) {
			for (const playbackNumber of Object.values(page.slots)) {
				const pages = usageByNumber.get(playbackNumber) ?? [];
				if (!pages.includes(page.number)) pages.push(page.number);
				usageByNumber.set(playbackNumber, pages);
			}
		}
		const normalizedSearch = search.toLowerCase();
		return Array.from({ length: 1000 }, (_, index) => ({
			number: index + 1,
			playback: byNumber.get(index + 1) ?? null,
			runtimeMaster:
				runtimeMaster(runtimes.get(index + 1)) ??
				legacyMasters.get(index + 1) ??
				null,
			usage: usageByNumber.get(index + 1) ?? [],
		})).filter(
			({ number, playback }) =>
				!search ||
				playback?.name.toLowerCase().includes(normalizedSearch) ||
				String(number).includes(search),
		);
	}, [legacyRuntime, pages, pool, runtimes, search]);
}

export function CuelistPool(props: CuelistPoolProps) {
	const { server, state, clearHold, startHold, click } =
		useCuelistPoolActions(props);
	const [search, setSearch] = useState("");
	const pool = useMemo(
		() =>
			(server.playbacks?.pool ?? []).filter(
				(definition) => definition.target.type === "cue_list",
			),
		[server.playbacks?.pool],
	);
	const runtimes = usePlaybackProjectionMap(
		props.active ? pool.map((playback) => playback.number) : [],
	);
	const filteredPool = usePoolSlots(
		pool,
		search,
		server.playbacks?.pages,
		runtimes,
		server.playbacks?.active,
	);
	const workflowMessage =
		state.cueListSetTarget != null
			? `Cuelist ${state.cueListSetTarget} selected · touch a playback fader to assign it.`
			: state.cueListSetArmed
				? "Select a Cuelist, then touch the playback fader where it should be assigned."
				: props.message;
	return (
		<div className="cuelist-window cuelist-pool-window pool-window">
			{!props.compact && (
				<WindowHeader
					title="Cuelist Pool"
					info={{
						primary: `${pool.length} / 1000 Cuelists`,
						secondary: workflowMessage ? (
							<span className="cuelist-workflow-status">{workflowMessage}</span>
						) : undefined,
					}}
					search={
						<SearchBar
							value={search}
							onChange={setSearch}
							ariaLabel="Search Cuelists"
							placeholder="Number or name"
						/>
					}
					actions={[]}
				/>
			)}
			{props.compact && workflowMessage && (
				<div className="pool-message">{workflowMessage}</div>
			)}
			<WindowScrollArea
				emptyState={
					filteredPool.length
						? null
						: {
								title: "No matching Cuelists",
								description: `No Cuelist matches “${search}”.`,
								icon: "⌕",
							}
				}
			>
				<ButtonGrid className="card-pool cuelist-pool-grid">
					{filteredPool.map((slot) => (
						<CuelistPoolSlot
							key={slot.number}
							{...slot}
							selectedCuelist={props.selectedCuelist}
							storeArmed={state.storeArmed}
							updateArmed={state.updateArmed}
							setTarget={state.cueListSetTarget === slot.number}
							onPointerDown={() => startHold(slot.number, slot.playback)}
							onPointerEnd={clearHold}
							onClick={() => click(slot.number, slot.playback)}
						/>
					))}
				</ButtonGrid>
			</WindowScrollArea>
			{props.settings}
		</div>
	);
}
