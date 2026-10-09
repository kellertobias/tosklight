import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";
import { ModalFrame } from "@tosklight/ui/modals";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
	type ResolvedPoolPresentation,
	type PoolCardSizing,
} from "@tosklight/ui/pools";
import { poolCardSizing } from "../../state/reducerHelpers";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import { useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type {
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
import {
	poolMutationTarget,
	poolObjectMutationCommand,
} from "../../features/controlSurfaceInteraction/poolCommandTarget";
import { useCueRecording } from "../../features/cueRecording/CueRecordingProvider";
import { useProgrammingUpdate } from "../../features/programmingUpdate/ProgrammingUpdateProvider";
import {
	type CueRecordPlan,
	recordTouchedTarget,
	smartChoiceRecordPlan,
} from "../../features/recordUpdateOptions/options";
import { useActiveShowId } from "../../features/deskSnapshot/DeskSnapshotState";
import { runtimeMaster } from "../../features/playbackRuntime/legacy";
import {
	useDirectCueListProjectionMap,
	usePlaybackProjectionMap,
	usePlaybackRuntimeActions,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../features/poolPresentation/poolPresentation";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../features/showObjects/ShowObjectsView";
import { useApp } from "../../state/AppContext";
import type { CuelistPoolEntry } from "../../features/playbackTopology/cuelistPoolCatalog";
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
	playback: CuelistPoolEntry | null;
	selectedCuelist: number | null;
	runtimeMaster: number | null;
	usage: number[];
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: boolean;
	offPending: boolean;
	onPointerDown: () => void;
	onPointerEnd: () => void;
	onClick: () => void;
	onContextMenu: () => void;
	onPreviewImage: () => void;
	presentation: ResolvedPoolPresentation;
}

const CUELIST_POOL_KINDS = ["cue_list", "playback", "playback_page"] as const;

function CuelistPoolSlot(props: PoolSlotProps) {
	const { number, playback, runtimeMaster, usage } = props;

	return (
		<div className="cuelist-card-slot">
			<PoolCard
				data-pool-slot-id={number}
				data-pool-position={props.poolPosition}
				className={`cuelist-card ${props.presentation.className} ${runtimeMaster != null ? "running" : ""} ${props.offPending && playback ? "command-target-off" : ""}`}
				style={props.presentation.style}
				aria-pressed={props.selectedCuelist === number && Boolean(playback)}
				model={{
					number,
					primary: playback?.cueList.name ?? "Empty",
					secondary: playback
						? props.offPending
							? "Tap to release Cuelist"
							: props.updateArmed
								? "Touch to choose Update mode"
								: `Cuelist · ${runtimeMaster != null ? `${Math.round(runtimeMaster * 100)}%` : "Off"}`
						: props.updateArmed
							? "Touch to check Update eligibility"
							: props.storeArmed
								? "Tap to record Cuelist"
								: "Press Rec first",
					details: playback
						? [
								...(playback.legacyAlias
									? [`Alias of Cuelist ${playback.canonicalNumber}`]
									: []),
								playback.assignment
									? `Playback ${playback.assignment.number}${usage.length ? ` · pages ${usage.join(", ")}` : " · no page assignment"}`
									: "Not assigned to a playback",
							]
						: undefined,
					color: playback?.assignment?.color,
					icon: playback?.assignment?.presentation_icon,
					image: playback?.assignment?.presentation_image
						? {
								src: playback.assignment?.presentation_image,
								alt: `${playback.cueList.name} presentation`,
							}
						: undefined,
					kind: "cuelist",
					status:
						runtimeMaster != null
							? `Active · ${Math.round(runtimeMaster * 100)}%`
							: undefined,
					states: props.presentation.states,
					workflow: props.offPending && playback ? "Off" : undefined,
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
			{playback?.assignment?.presentation_image && (
				<Button
					type="button"
					className="cuelist-preview-button"
					aria-label={`Open ${playback.cueList.name} preview`}
					onClick={props.onPreviewImage}
					onContextMenu={(event) => {
						event.preventDefault();
						props.onContextMenu();
					}}
				>
					<img src={playback.assignment?.presentation_image} alt="" />
				</Button>
			)}
		</div>
	);
}

function poolCueNumbers(
	cueLists: ReturnType<typeof useCueLists>,
	playback: CuelistPoolEntry | null,
) {
	if (!playback) return [];
	const cueListId = playback.cueList.id;
	const cueList = cueLists.find((item) => item.body.id === cueListId)?.body;
	return cueList?.cues.map((cue) => cue.number) ?? [];
}

function selectPoolSetSource(
	number: number,
	playback: CuelistPoolEntry | null,
	props: CuelistPoolProps,
	dispatch: ReturnType<typeof useApp>["dispatch"],
) {
	if (!playback) {
		props.onMessage(
			`Cuelist ${number} is empty · record it before assigning it to a playback.`,
		);
		return;
	}
	if (!props.builtIn) props.onSelectLocalCuelist(number);
	dispatch({ type: "SET_CUELIST_SET_TARGET", value: number });
	dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
}

function releasePoolCueList(
	playback: CuelistPoolEntry | null,
	runtimeActions: ReturnType<typeof usePlaybackRuntimeActions>,
	directRuntimes: ReturnType<typeof useDirectCueListProjectionMap>,
	assignedRuntimes: ReturnType<typeof usePlaybackProjectionMap>,
	props: CuelistPoolProps,
	command: ReturnType<typeof useCommandLineSurface>,
) {
	if (!playback) return;
	if (!runtimeActions) {
		props.onMessage(
			"Cuelist Off is unavailable while Playback authority is loading.",
		);
		return;
	}
	void runtimeActions
		.releaseCueListSource({
			identity:
				runtimeMaster(
					directRuntimes.projections.get(playback.cueList.id),
				) == null &&
				playback.assignment &&
				runtimeMaster(assignedRuntimes.get(playback.assignment.number)) !=
					null
					? {
							kind: "playback",
							playback_number: playback.assignment.number,
						}
					: { kind: "direct_cue_list", cue_list_id: playback.cueList.id },
			cueListId: playback.cueList.id,
		})
		.then(async (outcome) => {
			if (outcome) {
				props.onMessage("");
				await command.reset();
			} else {
				props.onMessage(
					"Cuelist Off was rejected; the OFF target remains armed.",
				);
			}
		});
	return;
}

function useCuelistPoolActions(props: CuelistPoolProps) {
	const command = useCommandLineSurface({
		enabled: props.active,
		observeCommand: true,
	});
	const cueRecording = useCueRecording();
	const programmingUpdate = useProgrammingUpdate();
	const runtimeActions = usePlaybackRuntimeActions();
	const catalog = useCuelistPool();
	const directRuntimes = useDirectCueListProjectionMap(
		catalog.map((entry) => entry.cueList.id),
		props.active,
	);
	const assignedRuntimes = usePlaybackProjectionMap(
		props.active
			? catalog.flatMap((entry) =>
					entry.assignment ? [entry.assignment.number] : [],
				)
			: [],
	);
	const cueLists = useCueLists();
	const { state, dispatch } = useApp();
	const [recordChoice, setRecordChoice] = useState<{
		number: number;
		playback: CuelistPoolEntry;
		cueNumber: string;
	} | null>(null);
	const holdTimer = useRef<number | null>(null);
	const held = useRef(false);
	const mutationTarget = poolMutationTarget(command.text);
	const offPending = /^OFF$/iu.test(command.text.trim());
	const clearHold = () => {
		if (holdTimer.current) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	const startHold = (number: number, playback: CuelistPoolEntry | null) => {
		if (!playback || !playback || state.updateArmed || offPending) return;
		held.current = false;
		holdTimer.current = window.setTimeout(() => {
			held.current = true;
			props.onOpenSettings(number);
		}, 650);
	};
	const openContextSettings = (
		number: number,
		playback: CuelistPoolEntry | null,
	) => {
		if (!playback) {
			props.onMessage(
				`Cuelist ${number} is empty · record it before opening settings.`,
			);
			return;
		}
		props.onMessage("");
		props.onOpenSettings(number);
	};
	const record = (
		number: number,
		playback: CuelistPoolEntry | null,
		plan: CueRecordPlan,
	) => {
		const settings = loadRecordSettings();

		void cueRecording
			?.record({
				target: { kind: "cuelist_pool", number },
				operation: plan.operation,
				...(plan.cueNumber ? { cueNumber: plan.cueNumber } : {}),
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
	};
	const recordTouched = (number: number, playback: CuelistPoolEntry | null) =>
		recordTouchedTarget({
			commandText: command.read().text,
			update: programmingUpdate,
			cueNumbers: poolCueNumbers(cueLists, playback),
			record: (plan) => record(number, playback, plan),
			ask: (cueNumber) =>
				playback && setRecordChoice({ number, playback, cueNumber }),
		});
	const click = (number: number, playback: CuelistPoolEntry | null) => {
		if (held.current) {
			held.current = false;
			return;
		}
		if (offPending) {
			releasePoolCueList(
				playback,
				runtimeActions,
				directRuntimes,
				assignedRuntimes,
				props,
				command,
			);
			return;
		}
		if (state.updateArmed) {
			const objectId = playback ? playback.cueList.id : String(number);
			requestUpdateTarget(cueUpdateTarget(objectId));
			return;
		}
		const mutation = poolObjectMutationCommand(
			mutationTarget?.operation === "delete" ? null : mutationTarget,
			"CUELIST",
			number,
			playback !== null,
		);
		if (mutation) {
			if (mutation.kind === "execute") void command.execute(mutation.command);
			else void command.replace(mutation.command, false);
			return;
		}
		if (state.storeArmed) {
			void recordTouched(number, playback);
			return;
		}
		if (/^ASSIGN$/i.test(command.read().text.trim())) {
			if (!playback) {
				props.onMessage(
					`Cuelist ${number} is empty · record it before assigning it to a playback.`,
				);
				return;
			}
			void command.replace(`ASSIGN CUELIST ${number}`);
			return;
		}
		if (state.cueListSetArmed) {
			selectPoolSetSource(number, playback, props, dispatch);
			return;
		}
		if (!playback) return;
		props.onMessage("");
		props.onOpenCuelist(number);
	};
	return {
		state,
		offPending,
		assignArmed: /^ASSIGN$/i.test(command.read().text.trim()),
		mutationTarget,
		clearHold,
		startHold,
		click,
		openContextSettings,
		recordChoice,
		resolveRecordChoice: (choice: "add" | "merge" | "overwrite" | null) => {
			const pending = recordChoice;
			setRecordChoice(null);
			if (pending && choice)
				record(
					pending.number,
					pending.playback,
					smartChoiceRecordPlan(choice, pending.cueNumber),
				);
		},
	};
}

function usePoolSlots(
	pool: CuelistPoolEntry[],
	search: string,
	pages: PlaybackPage[] | undefined,
	runtimes: ReadonlyMap<
		string,
		| import("../../features/playbackRuntime/contracts").PlaybackProjection
		| undefined
	>,
	assignedRuntimes: ReturnType<typeof usePlaybackProjectionMap>,
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
			runtimeMaster:
				runtimeMaster(
					runtimes.get(byNumber.get(index + 1)?.cueList.id ?? ""),
				) ??
				runtimeMaster(
					assignedRuntimes.get(
						byNumber.get(index + 1)?.assignment?.number ?? -1,
					),
				) ??
				null,
			usage:
				usageByNumber.get(byNumber.get(index + 1)?.assignment?.number ?? -1) ??
				[],
		})).filter(
			({ number, playback }) =>
				!search ||
				playback?.cueList.name.toLowerCase().includes(normalizedSearch) ||
				String(number).includes(search),
		);
	}, [pages, pool, runtimes, assignedRuntimes, search]);
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
			primary: slot.playback?.cueList.name ?? "Empty",
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
	mutationTarget,
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
	mutationTarget: ReturnType<typeof poolMutationTarget>;
}) {
	const itemId = slot.playback ? slot.playback.cueList.id : String(slot.number);
	return resolveConfiguredPoolPresentation(configuration, {
		showId,
		surfaceKey,
		objectType: "cuelist",
		itemColorKey: itemId,
		itemColor: slot.playback?.assignment?.color,
		states: [
			...(!slot.playback ? (["empty"] as const) : []),
			...(slot.runtimeMaster != null ? (["active"] as const) : []),
			...(selectedCuelist === slot.number && slot.playback
				? (["selected"] as const)
				: []),
			...(storeArmed && (!slot.playback || slot.playback)
				? (["record-target"] as const)
				: []),
			...(storeArmed && (!slot.playback || slot.playback)
				? (["store-target"] as const)
				: []),
			...(updateArmed ? (["update-target"] as const) : []),
			...(slot.playback && (setArmed || setTarget)
				? (["set-target"] as const)
				: []),
			...(poolObjectMutationCommand(
				mutationTarget?.operation === "delete" ? null : mutationTarget,
				"CUELIST",
				slot.number,
				slot.playback !== null,
			)
				? ([`${mutationTarget?.operation ?? "copy"}-target`] as const)
				: []),
		],
	});
}

function CuelistPoolCards({
	slots,
	search,
	cardSizing,
	configuration,
	showId,
	surfaceKey,
	selectedCuelist,
	storeArmed,
	updateArmed,
	setTarget,
	setArmed,
	mutationTarget,
	offPending,
	startHold,
	clearHold,
	click,
	openContextSettings,
	onPreviewImage,
}: {
	slots: readonly CuelistPoolItem[];
	search: string;
	cardSizing: PoolCardSizing;
	configuration: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	selectedCuelist: number | null;
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: number | null;
	setArmed: boolean;
	mutationTarget: ReturnType<typeof poolMutationTarget>;
	offPending: boolean;
	startHold: (number: number, playback: CuelistPoolEntry | null) => void;
	clearHold: () => void;
	click: (number: number, playback: CuelistPoolEntry | null) => void;
	openContextSettings: (
		number: number,
		playback: CuelistPoolEntry | null,
	) => void;
	onPreviewImage: (number: number, playback: CuelistPoolEntry) => void;
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
			mutationTarget,
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
				offPending={offPending}
				onPointerDown={() => startHold(slot.number, slot.playback)}
				onPointerEnd={clearHold}
				onClick={() => click(slot.number, slot.playback)}
				onContextMenu={() => {
					// The right button's pointerdown started a hold, and its pointerup lands
					// on the Settings dialog, so the hold would reopen Settings after it closes.
					clearHold();
					openContextSettings(slot.number, slot.playback);
				}}
				onPreviewImage={() =>
					slot.playback && onPreviewImage(slot.number, slot.playback)
				}
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
				cardSizing={cardSizing}
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
				onSearch,
				ariaLabel: "Search Cuelists",
				placeholder: "Number or name",
			}}
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
	const {
		state,
		clearHold,
		startHold,
		click,
		openContextSettings,
		recordChoice,
		resolveRecordChoice,
		assignArmed,
		mutationTarget,
		offPending,
	} = useCuelistPoolActions(props);
	const [search, setSearch] = useState("");
	const [preview, setPreview] = useState<{
		number: number;
		name: string;
		src: string;
	} | null>(null);
	const [colorSettingsAnchor, setColorSettingsAnchor] =
		useState<DOMRect | null>(null);
	const pool = useCuelistPool();
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const surfaceKey = poolSurfaceKey(showId, "cuelist", props.paneId);
	const pages = usePlaybackPages();
	useShowObjectKindsView(CUELIST_POOL_KINDS, props.active);
	const runtimes = useDirectCueListProjectionMap(
		pool.map((entry) => entry.cueList.id),
		props.active,
	).projections;
	const assignedRuntimes = usePlaybackProjectionMap(
		props.active
			? pool.flatMap((entry) =>
					entry.assignment ? [entry.assignment.number] : [],
				)
			: [],
	);
	const filteredPool = usePoolSlots(
		pool,
		search,
		pages.map((object) => object.body),
		runtimes,
		assignedRuntimes,
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
				cardSizing={poolCardSizing(state)}
				configuration={poolPresentation}
				showId={showId}
				surfaceKey={surfaceKey}
				selectedCuelist={props.selectedCuelist}
				storeArmed={state.storeArmed}
				updateArmed={state.updateArmed}
				setTarget={state.cueListSetTarget}
				setArmed={state.cueListSetArmed || assignArmed}
				mutationTarget={mutationTarget}
				offPending={offPending}
				startHold={startHold}
				clearHold={clearHold}
				click={click}
				openContextSettings={openContextSettings}
				onPreviewImage={(number, playback) => {
					if (!playback.assignment?.presentation_image) return;
					setPreview({
						number,
						name: playback.cueList.name,
						src: playback.assignment?.presentation_image,
					});
				}}
			/>
			{props.settings}
			{recordChoice &&
				createPortal(
					<ModalRegistration onClose={() => resolveRecordChoice(null)}>
						<div className="stacked-modal-layer cue-record-choice-layer">
							<section
								className="nested-modal cue-record-choice-modal"
								role="dialog"
								aria-modal="true"
								aria-label="Record Cue choice"
							>
								<ModalTitleBar title={`Record Cue ${recordChoice.cueNumber}`} />
								<p>This Cuelist contains one Cue. Choose how to record it.</p>
								<div className="command-choice-actions">
									<Button onClick={() => resolveRecordChoice("add")}>
										Add Cue
									</Button>
									<Button onClick={() => resolveRecordChoice("merge")}>
										Merge Cue
									</Button>
									<Button onClick={() => resolveRecordChoice("overwrite")}>
										Overwrite Cue
									</Button>
									<Button onClick={() => resolveRecordChoice(null)}>
										Cancel
									</Button>
								</div>
							</section>
						</div>
					</ModalRegistration>,
					document.body,
				)}
			{preview && (
				<CuelistPreviewModal
					preview={preview}
					onClose={() => setPreview(null)}
				/>
			)}
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

function CuelistPreviewModal({
	preview,
	onClose,
}: {
	preview: { number: number; name: string; src: string };
	onClose: () => void;
}) {
	return (
		<ModalFrame
			id={`cuelist-preview-${preview.number}`}
			ariaLabel={`${preview.name} preview image`}
			title={`Cuelist ${preview.number} · ${preview.name}`}
			closeLabel="Close Cuelist preview"
			dialogClassName="cuelist-preview-modal"
			onClose={onClose}
		>
			<div className="cuelist-preview-modal-body">
				<img src={preview.src} alt={`${preview.name} preview`} />
			</div>
		</ModalFrame>
	);
}
