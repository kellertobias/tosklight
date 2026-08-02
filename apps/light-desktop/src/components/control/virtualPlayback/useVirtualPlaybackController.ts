import { useEffect, useMemo, useState } from "react";
import type { PlaybackDefinition } from "../../../api/types";
import {
	isVirtualPlaybackNumberForPage,
	virtualPlaybackBankStart,
	virtualPlaybackNumber,
	virtualPlaybackPage,
} from "../../../api/virtualPlaybackAddress";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
	useVirtualPlaybackProjectionMap,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackTopologyView } from "../../../features/playbackTopology/PlaybackTopologyView";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import { useApp } from "../../../state/AppContext";
import { normalizePlaybackTopology } from "../PlaybackConfigurationModal";
import { emptyConfiguration } from "../PlaybackFaderBank";
import { useVirtualPlaybackSurfaceZones } from "./useVirtualPlaybackSurfaceZones";
import {
	MAX_VIRTUAL_PLAYBACK_CELLS,
	validPlaybackSlot,
} from "./VirtualPlaybackGrid";

interface ConfigurationState {
	playback: PlaybackDefinition;
	page: number;
	slot: number;
	empty: boolean;
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
	expectedPlaybackRevision: number;
	expectedPlaybackObjectId: string | null;
}

export function useVirtualPlaybackController(
	paneId: string | undefined,
	active: boolean,
) {
	const { state, dispatch } = useApp();
	const topology = usePlaybackTopologyView(active);
	const topologyActions = usePlaybackTopologyActions();
	const runtimeActions = usePlaybackRuntimeActions();
	const playbackDesk = usePlaybackDeskView(active);
	const runtimeStatus = usePlaybackRuntimeStatus(active);
	const surfaceId = paneId ?? "builtin-virtual-playbacks";
	const pane = state.desks
		.flatMap((desk) => desk.panes)
		.find((candidate) => candidate.id === paneId);
	const rows = pane?.virtualPlaybackRows ?? 2;
	const columns = pane?.virtualPlaybackColumns ?? 2;
	const pageMode = pane?.virtualPlaybackPageMode ?? "follow_main";
	const pinnedPage = pane?.virtualPlaybackPinnedPage ?? 1;
	const pageNumber =
		pageMode === "pinned" ? pinnedPage : (playbackDesk?.active_page ?? null);
	const authorityReady =
		active &&
		topology.ready &&
		runtimeStatus.status === "ready" &&
		playbackDesk !== null &&
		pageNumber !== null;
	const pageObject = topology.pages.find(
		(candidate) => candidate.body.number === pageNumber,
	);
	const page = pageObject?.body;
	const playbacks = useMemo(
		() => new Map(topology.playbacks.map(({ body }) => [body.number, body])),
		[topology.playbacks],
	);
	const cueLists = useMemo(
		() => new Map(topology.cueLists.map(({ body }) => [body.id, body])),
		[topology.cueLists],
	);
	const virtualAddresses = useMemo(
		() => mappedVirtualPlaybackAddresses(page, rows * columns),
		[columns, page, rows],
	);
	// Activate the runtime for the current page's playbacks as soon as the topology and page are
	// known (playbackNumbers is empty until then). Gating this on authorityReady deadlocked the
	// pane: authorityReady needs the runtime "ready", but the runtime only becomes ready once it is
	// activated for these identities, so a Virtual Playbacks pane opened on its own stayed stuck at
	// "Loading Virtual Playbacks…".
	const runtimes = useVirtualPlaybackProjectionMap(virtualAddresses);
	const zones = useVirtualPlaybackSurfaceZones({
		surfaceId,
		active,
		authorityReady,
		pageMode:
			pageMode === "pinned"
				? { type: "pinned", page: pinnedPage }
				: { type: "follow_main" },
	});
	const interactions = useVirtualPlaybackInteractions({
		surfaceId,
		state,
		dispatch,
		topology,
		pageNumber,
		rows,
		columns,
		zones,
	});
	return {
		state,
		dispatch,
		topology,
		topologyActionError: topologyActions?.error?.message ?? null,
		runtimeStatus,
		runtimeActions,
		pageNumber,
		page,
		pageObject,
		rows,
		columns,
		playbacks,
		cueLists,
		runtimes,
		zones,
		authorityReady,
		...interactions,
	};
}

interface InteractionOptions {
	surfaceId: string;
	state: ReturnType<typeof useApp>["state"];
	dispatch: ReturnType<typeof useApp>["dispatch"];
	topology: ReturnType<typeof usePlaybackTopologyView>;
	pageNumber: number | null;
	rows: number;
	columns: number;
	zones: ReturnType<typeof useVirtualPlaybackSurfaceZones>;
}

function openVirtualPlaybackConfiguration(
	options: InteractionOptions,
	playback: PlaybackDefinition | null,
	slot: number,
	setConfiguration: (configuration: ConfigurationState) => void,
) {
	if (!validPlaybackSlot(slot) || options.pageNumber == null) return;
	const playbackNumber = virtualPlaybackNumber(options.pageNumber, slot);
	const next =
		playback ??
		({
			...emptyConfiguration(
				options.pageNumber,
				playbackNumber,
				1,
				false,
				options.topology.cueLists[0]?.body.id ?? "",
			),
			buttons: ["go", "none", "none"],
		} satisfies PlaybackDefinition);
	setConfiguration({
		playback: normalizePlaybackTopology(
			{ ...next, button_count: 1, has_fader: false },
			1,
			false,
		),
		page: options.pageNumber,
		slot,
		empty: !playback,
		expectedPageRevision:
			options.topology.pages.find(
				(candidate) => candidate.body.number === options.pageNumber,
			)?.revision ?? 0,
		expectedPageObjectId:
			options.topology.pages.find(
				(candidate) => candidate.body.number === options.pageNumber,
			)?.id ?? null,
		expectedPlaybackRevision:
			options.topology.playbacks.find(
				(candidate) => candidate.body.number === playback?.number,
			)?.revision ?? 0,
		expectedPlaybackObjectId:
			options.topology.playbacks.find(
				(candidate) => candidate.body.number === playback?.number,
			)?.id ?? null,
	});
	options.dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
	options.dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	options.dispatch({ type: "SET_SHIFT_ARMED", value: false });
}

function selectedZonePlaybackCount(
	options: InteractionOptions,
	zoneEdit: ReturnType<typeof useApp>["state"]["virtualPlaybackZoneEdit"],
	selectedSlots: number[],
) {
	if (!zoneEdit || options.pageNumber == null) return selectedSlots.length;
	const hiddenCount =
		options.zones.zones
			.find((candidate) => candidate.id === zoneEdit.zoneId)
			?.playbackNumbers.filter(
				(number) => virtualPlaybackPage(number) !== options.pageNumber,
			).length ?? 0;
	return selectedSlots.length + hiddenCount;
}

function useVirtualPlaybackInteractions(options: InteractionOptions) {
	const [configuration, setConfiguration] = useState<ConfigurationState | null>(
		null,
	);
	const [selectedSlots, setSelectedSlots] = useState<number[]>([]);
	const [creatingZone, setCreatingZone] = useState(false);
	const [zoneName, setZoneName] = useState("");
	const zoneEdit = options.state.virtualPlaybackZoneEdit;
	const configurationArmed =
		options.state.playbackSetArmed || options.state.cueListSetArmed;
	useEffect(() => {
		setConfiguration(null);
		if (zoneEdit) return;
		setSelectedSlots([]);
		setCreatingZone(false);
		setZoneName("");
	}, [options.pageNumber, zoneEdit]);
	useEffect(() => {
		if (options.topology.ready) return;
		setConfiguration(null);
	}, [options.topology.ready]);
	useEffect(() => {
		if (!zoneEdit || options.pageNumber == null) return;
		const pageNumber = options.pageNumber;
		const start = virtualPlaybackBankStart(pageNumber);
		setSelectedSlots(
			zoneEdit.playbackNumbers
				.filter((number) => isVirtualPlaybackNumberForPage(pageNumber, number))
				.map((number) => number - start + 1),
		);
		options.dispatch({ type: "SET_SHIFT_ARMED", value: true });
	}, [options.dispatch, options.pageNumber, zoneEdit]);

	useEffect(() => {
		if (zoneEdit) return;
		const lastSlot = Math.min(
			options.rows * options.columns,
			MAX_VIRTUAL_PLAYBACK_CELLS,
		);
		setSelectedSlots((current) => current.filter((slot) => slot <= lastSlot));
	}, [options.rows, options.columns, zoneEdit]);

	const openConfiguration = (
		playback: PlaybackDefinition | null,
		slot: number,
	) =>
		openVirtualPlaybackConfiguration(
			options,
			playback,
			slot,
			setConfiguration,
		);

	const toggleZoneSlot = (slot: number) => {
		if (!options.zones.ready || !validPlaybackSlot(slot)) return;
		setSelectedSlots((current) =>
			current.includes(slot)
				? current.filter((candidate) => candidate !== slot)
				: [...current, slot].sort((left, right) => left - right),
		);
	};

	const createZone = async (inputName = zoneName) => {
		const name = inputName.trim();
		if (
			zoneEdit ||
			!options.zones.ready ||
			options.pageNumber == null ||
			!name ||
			selectedSlots.length < 2
		)
			return;
		const pageNumber = options.pageNumber;
		const zone: VirtualPlaybackZone = {
			id: crypto.randomUUID(),
			name,
			playbackNumbers: selectedSlots.map((slot) =>
				virtualPlaybackNumber(pageNumber, slot),
			),
		};
		if (!(await options.zones.persist([...options.zones.zones, zone]))) return;
		options.dispatch({ type: "SET_SHIFT_ARMED", value: false });
		setSelectedSlots([]);
		setZoneName("");
		setCreatingZone(false);
	};

	const cancelZoneSelection = () => {
		setSelectedSlots([]);
		setCreatingZone(false);
		setZoneName("");
		options.dispatch({ type: "SET_SHIFT_ARMED", value: false });
		if (zoneEdit)
			options.dispatch({ type: "SET_VIRTUAL_PLAYBACK_ZONE_EDIT", edit: null });
	};

	const updateZone = async () => {
		if (!zoneEdit || !options.zones.ready || options.pageNumber == null) return;
		const pageNumber = options.pageNumber;
		const current = options.zones.zones.find(
			(candidate) => candidate.id === zoneEdit.zoneId,
		);
		if (!current) return;
		const hiddenNumbers = current.playbackNumbers.filter(
			(number) => virtualPlaybackPage(number) !== pageNumber,
		);
		if (hiddenNumbers.length + selectedSlots.length < 2) return;
		const visibleNumbers = selectedSlots.map((slot) =>
			virtualPlaybackNumber(pageNumber, slot),
		);
		const next = options.zones.zones.map((candidate) =>
			candidate.id === zoneEdit.zoneId
				? {
						...candidate,
						playbackNumbers: [...hiddenNumbers, ...visibleNumbers].sort(
							(left, right) => left - right,
						),
					}
				: candidate,
		);
		if (!(await options.zones.persist(next))) return;
		cancelZoneSelection();
	};
	const selectedPlaybackCount = selectedZonePlaybackCount(options, zoneEdit, selectedSlots);

	return {
		configuration:
			configuration?.page === options.pageNumber && options.topology.ready
				? configuration
				: null,
		setConfiguration,
		selectedSlots,
		selectedPlaybackCount,
		setSelectedSlots,
		creatingZone,
		setCreatingZone,
		zoneName,
		setZoneName,
		zoneEdit,
		configurationArmed,
		openConfiguration,
		toggleZoneSlot,
		createZone,
		cancelZoneSelection,
		updateZone,
	};
}

function mappedVirtualPlaybackAddresses(
	page:
		| { number: number; virtual_playbacks?: Record<string, PlaybackDefinition> }
		| undefined,
	cellCount: number,
) {
	if (!page?.virtual_playbacks) return [];
	return Object.keys(page.virtual_playbacks)
		.map(Number)
		.filter(
			(number) =>
				Number.isSafeInteger(number) &&
				isVirtualPlaybackNumberForPage(page.number, number) &&
				number <
					virtualPlaybackNumber(
						page.number,
						Math.min(cellCount, MAX_VIRTUAL_PLAYBACK_CELLS),
					) +
						1,
		)
		.map((playbackNumber) => ({ page: page.number, playbackNumber }));
}
