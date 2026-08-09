import {
	DEFAULT_POOL_CARD_MINIMUM_WIDTH,
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
	type ResolvedPoolPresentation,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import { useMemo, useRef, useState } from "react";
import type {
	PlaybackDefinition,
	PlaybackPage,
	PoolPresentationConfiguration,
} from "../../api/types";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import {
	cueUpdateTarget,
	requestUpdateTarget,
} from "../../components/control/updateWorkflow";
import { loadRecordSettings } from "../../components/setup/ProgrammerDefaults";
import { PoolColorSettings } from "../../components/shared/PoolColorSettings";
import { useCueRecording } from "../../features/cueRecording/CueRecordingProvider";
import { useActiveShowId } from "../../features/deskSnapshot/DeskSnapshotState";
import { runtimeMaster } from "../../features/playbackRuntime/legacy";
import { usePlaybackProjectionMap } from "../../features/playbackRuntime/PlaybackRuntimeView";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../features/poolPresentation/poolPresentation";
import { usePlaybackPages } from "../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../features/showObjects/ShowObjectsView";
import { useApp } from "../../state/AppContext";
import { useCuelistPool } from "./useCuelistSelection";

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
	paneId?: string;
}

interface PoolSlotProps {
	number: number;
	poolPosition: number;
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
	onContextMenu: () => void;
	presentation: ResolvedPoolPresentation;
}

const CUELIST_POOL_KINDS = ["cue_list", "playback", "playback_page"] as const;

function CuelistPoolSlot(props: PoolSlotProps) {
	const { number, playback, runtimeMaster, usage } = props;
	return (
		<PoolCard
			data-pool-slot-id={number}
			data-pool-position={props.poolPosition}
			className={`cuelist-card ${props.presentation.className} ${runtimeMaster != null ? "running" : ""}`}
			style={props.presentation.style}
			aria-pressed={props.selectedCuelist === number && Boolean(playback)}
			model={{
				number,
				primary: playback?.name ?? "Empty",
				secondary: playback
					? props.updateArmed
						? "Touch to choose Update mode"
						: `Cuelist · ${runtimeMaster != null ? `${Math.round(runtimeMaster * 100)}%` : "Off"}`
					: props.updateArmed
						? "Touch to check Update eligibility"
						: props.storeArmed
							? "Tap to record Cuelist"
							: "Press Rec first",
				details: playback
					? [
							usage.length
								? `Playbacks on pages ${usage.join(", ")}`
								: "Not assigned to a playback",
						]
					: undefined,
				color: playback?.color,
				icon: playback?.presentation_icon,
				image: playback?.presentation_image
					? {
							src: playback.presentation_image,
							alt: `${playback.name} presentation`,
						}
					: undefined,
				kind: "cuelist",
				status:
					runtimeMaster != null
						? `Active · ${Math.round(runtimeMaster * 100)}%`
						: undefined,
				states: props.presentation.states,
			}}
			onPointerDown={props.onPointerDown}
			onPointerUp={props.onPointerEnd}
			onPointerCancel={props.onPointerEnd}
			onContextMenu={(event) => {
				event.preventDefault();
				props.onContextMenu();
			}}
			onClick={props.onClick}
		/>
	);
}

function useCuelistPoolActions(props: CuelistPoolProps) {
	const command = useCommandLineSurface({
		enabled: props.active,
		observeCommand: false,
	});
	const cueRecording = useCueRecording();
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
	const selectSetSource = (
		number: number,
		playback: PlaybackDefinition | null,
	) => {
		if (!playback) {
			props.onMessage(
				`Cuelist ${number} is empty · record it before assigning it to a playback.`,
			);
			return;
		}
		if (!props.builtIn) props.onSelectLocalCuelist(number);
		dispatch({ type: "SET_CUELIST_SET_TARGET", value: number });
		dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
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
			const settings = loadRecordSettings();
			void cueRecording
				?.record({
					target: { kind: "pool", playbackNumber: number },
					operation: "overwrite",
					timing: {},
					cueOnly: settings.cueOnly,
					capturePolicy: "current_capture",
					activationPolicy: "hold",
				})
				.then(async (outcome) => {
					if (!outcome) return;
					dispatch({ type: "SET_STORE_ARMED", value: false });
					await command.reset();
				});
			return;
		}
		if (state.cueListSetArmed) {
			selectSetSource(number, playback);
			return;
		}
		if (!playback) return;
		props.onMessage("");
		props.onOpenCuelist(number);
	};
	return { state, clearHold, startHold, click, selectSetSource };
}

function usePoolSlots(
	pool: PlaybackDefinition[],
	search: string,
	pages: PlaybackPage[] | undefined,
	runtimes: ReturnType<typeof usePlaybackProjectionMap>,
) {
	return useMemo(() => {
		const byNumber = new Map(
			pool.map((playback) => [playback.number, playback]),
		);
		const usageByNumber = new Map<number, number[]>();
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
			runtimeMaster: runtimeMaster(runtimes.get(index + 1)) ?? null,
			usage: usageByNumber.get(index + 1) ?? [],
		})).filter(
			({ number, playback }) =>
				!search ||
				playback?.name.toLowerCase().includes(normalizedSearch) ||
				String(number).includes(search),
		);
	}, [pages, pool, runtimes, search]);
}

type CuelistPoolItem = ReturnType<typeof usePoolSlots>[number];

function poolSlotViewModels(
	slots: readonly CuelistPoolItem[],
): PoolSlotViewModel<number>[] {
	return slots.map((slot) => ({
		id: slot.number,
		position: slot.number - 1,
		card: {
			number: slot.number,
			primary: slot.playback?.name ?? "Empty",
		},
	}));
}

function resolveCuelistPresentation({
	slot,
	configuration,
	showId,
	surfaceKey,
	selectedCuelist,
	storeArmed,
	updateArmed,
	setTarget,
	setArmed,
}: {
	slot: CuelistPoolItem;
	configuration: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	selectedCuelist: number | null;
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: boolean;
	setArmed: boolean;
}) {
	const itemId =
		slot.playback?.target.type === "cue_list"
			? slot.playback.target.cue_list_id
			: String(slot.number);
	return resolveConfiguredPoolPresentation(configuration, {
		showId,
		surfaceKey,
		objectType: "cuelist",
		itemColorKey: itemId,
		itemColor: slot.playback?.color,
		states: [
			...(!slot.playback ? (["empty"] as const) : []),
			...(slot.runtimeMaster != null ? (["active"] as const) : []),
			...(selectedCuelist === slot.number && slot.playback
				? (["selected"] as const)
				: []),
			...(storeArmed ? (["record-target"] as const) : []),
			...(storeArmed ? (["store-target"] as const) : []),
			...(updateArmed ? (["update-target"] as const) : []),
			...(slot.playback && (setArmed || setTarget)
				? (["set-target"] as const)
				: []),
		],
	});
}

function CuelistPoolCards({
	slots,
	search,
	configuration,
	showId,
	surfaceKey,
	selectedCuelist,
	storeArmed,
	updateArmed,
	setTarget,
	setArmed,
	startHold,
	clearHold,
	click,
	selectSetSource,
}: {
	slots: readonly CuelistPoolItem[];
	search: string;
	configuration: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	selectedCuelist: number | null;
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: number | null;
	setArmed: boolean;
	startHold: (number: number, playback: PlaybackDefinition | null) => void;
	clearHold: () => void;
	click: (number: number, playback: PlaybackDefinition | null) => void;
	selectSetSource: (
		number: number,
		playback: PlaybackDefinition | null,
	) => void;
}) {
	const poolSlots = poolSlotViewModels(slots);
	const renderSlot = (slot: CuelistPoolItem, poolPosition: number) => {
		const isSetTarget = setTarget === slot.number;
		const presentation = resolveCuelistPresentation({
			slot,
			configuration,
			showId,
			surfaceKey,
			selectedCuelist,
			storeArmed,
			updateArmed,
			setTarget: isSetTarget,
			setArmed,
		});
		return (
			<CuelistPoolSlot
				key={slot.number}
				{...slot}
				poolPosition={poolPosition}
				selectedCuelist={selectedCuelist}
				storeArmed={storeArmed}
				updateArmed={updateArmed}
				setTarget={isSetTarget}
				onPointerDown={() => startHold(slot.number, slot.playback)}
				onPointerEnd={clearHold}
				onClick={() => click(slot.number, slot.playback)}
				onContextMenu={() => selectSetSource(slot.number, slot.playback)}
				presentation={presentation}
			/>
		);
	};
	return (
		<WindowScrollArea
			emptyState={
				slots.length
					? null
					: {
							title: "No matching Cuelists",
							description: `No Cuelist matches “${search}”.`,
							icon: "⌕",
						}
			}
		>
			<PoolGrid
				className="cuelist-pool-grid"
				minimumCardWidth={DEFAULT_POOL_CARD_MINIMUM_WIDTH}
				slots={poolSlots}
				slotCount={search ? undefined : 1000}
				fillEmptySlots={!search}
				emptySlot={emptyCuelistSlot}
				renderSlot={(_, index) => renderSlot(slots[index], index)}
			/>
		</WindowScrollArea>
	);
}

function emptyCuelistSlot(index: number): PoolSlotViewModel<number> {
	return {
		id: index + 1,
		position: index,
		card: { number: index + 1, primary: "Empty", states: ["empty"] },
	};
}

function CuelistPoolHeader({
	count,
	workflowMessage,
	search,
	onSearch,
	onSettings,
}: {
	count: number;
	workflowMessage: string;
	search: string;
	onSearch: (value: string) => void;
	onSettings: (anchor: DOMRect) => void;
}) {
	return (
		<WindowHeader
			title="Cuelist Pool"
			info={{
				primary: `${count} / 1000 Cuelists`,
				secondary: workflowMessage ? (
					<span className="cuelist-workflow-status">{workflowMessage}</span>
				) : undefined,
			}}
			search={{
				value: search,
				ariaLabel: "Search Cuelists",
				placeholder: "Number or name",
			}}
			onSearch={onSearch}
			actions={[]}
			settings
			onSettings={(button) => onSettings(button.getBoundingClientRect())}
		/>
	);
}

function CuelistPoolSettings({
	anchor,
	paneId,
	onClose,
}: {
	anchor: DOMRect;
	paneId?: string;
	onClose: () => void;
}) {
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title="Cuelist Pool Settings"
			onClose={onClose}
			tabs={[
				{
					id: "colors",
					label: "Colors",
					content: <PoolColorSettings objectType="cuelist" paneId={paneId} />,
				},
			]}
		/>
	);
}

export function CuelistPool(props: CuelistPoolProps) {
	const { state, clearHold, startHold, click, selectSetSource } =
		useCuelistPoolActions(props);
	const [search, setSearch] = useState("");
	const [colorSettingsAnchor, setColorSettingsAnchor] =
		useState<DOMRect | null>(null);
	const pool = useCuelistPool();
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const surfaceKey = poolSurfaceKey(showId, "cuelist", props.paneId);
	const pages = usePlaybackPages();
	useShowObjectKindsView(CUELIST_POOL_KINDS, props.active);
	const runtimes = usePlaybackProjectionMap(
		props.active ? pool.map((playback) => playback.number) : [],
	);
	const filteredPool = usePoolSlots(
		pool,
		search,
		pages.map((object) => object.body),
		runtimes,
	);
	const workflowMessage =
		state.cueListSetTarget != null
			? `Cuelist ${state.cueListSetTarget} selected · touch a playback fader to assign it.`
			: props.message
				? props.message
			: state.cueListSetArmed
				? "Select a Cuelist, then touch the playback fader where it should be assigned."
				: "";
	return (
		<div className="cuelist-window cuelist-pool-window pool-window">
			{!props.compact && (
				<CuelistPoolHeader
					count={pool.length}
					workflowMessage={workflowMessage}
					search={search}
					onSearch={setSearch}
					onSettings={setColorSettingsAnchor}
				/>
			)}
			{props.compact && workflowMessage && (
				<div className="pool-message">{workflowMessage}</div>
			)}
			<CuelistPoolCards
				slots={filteredPool}
				search={search}
				configuration={poolPresentation}
				showId={showId}
				surfaceKey={surfaceKey}
				selectedCuelist={props.selectedCuelist}
				storeArmed={state.storeArmed}
				updateArmed={state.updateArmed}
				setTarget={state.cueListSetTarget}
				setArmed={state.cueListSetArmed}
				startHold={startHold}
				clearHold={clearHold}
				click={click}
				selectSetSource={selectSetSource}
			/>
			{props.settings}
			{colorSettingsAnchor && (
				<CuelistPoolSettings
					anchor={colorSettingsAnchor}
					paneId={props.paneId}
					onClose={() => setColorSettingsAnchor(null)}
				/>
			)}
		</div>
	);
}
