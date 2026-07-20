import { useEffect, useMemo, useState } from "react";
import type { PlaybackDefinition } from "../../../api/types";
import {
	usePlaybackDeskView,
	usePlaybackProjectionMap,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackTopologyView } from "../../../features/playbackTopology/PlaybackTopologyView";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import { useApp } from "../../../state/AppContext";
import { emptyConfiguration } from "../PlaybackFaderBank";
import { normalizePlaybackTopology } from "../PlaybackConfigurationModal";
import {
	MAX_PLAYBACK_SLOT,
	validPlaybackSlot,
} from "./VirtualPlaybackGrid";
import { useVirtualPlaybackSurfaceZones } from "./useVirtualPlaybackSurfaceZones";

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
	const pageNumber = playbackDesk?.active_page ?? null;
	const authorityReady =
		active &&
		topology.ready &&
		runtimeStatus.status === "ready" &&
		playbackDesk !== null &&
		pageNumber !== null;
	const page = topology.pages.find(
		(candidate) => candidate.body.number === pageNumber,
	)?.body;
	const playbacks = useMemo(
		() => new Map(topology.playbacks.map(({ body }) => [body.number, body])),
		[topology.playbacks],
	);
	const cueLists = useMemo(
		() => new Map(topology.cueLists.map(({ body }) => [body.id, body])),
		[topology.cueLists],
	);
	const playbackNumbers = useMemo(
		() => mappedPlaybackNumbers(page?.slots, rows * columns),
		[columns, page, rows],
	);
	const runtimes = usePlaybackProjectionMap(
		authorityReady ? playbackNumbers : [],
	);
	const zones = useVirtualPlaybackSurfaceZones({
		surfaceId,
		active,
		authorityReady,
	});
	const interactions = useVirtualPlaybackInteractions({
		state,
		dispatch,
		topology,
		topologyActions,
		playbacks,
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
	state: ReturnType<typeof useApp>["state"];
	dispatch: ReturnType<typeof useApp>["dispatch"];
	topology: ReturnType<typeof usePlaybackTopologyView>;
	topologyActions: ReturnType<typeof usePlaybackTopologyActions>;
	playbacks: ReadonlyMap<number, PlaybackDefinition>;
	pageNumber: number | null;
	rows: number;
	columns: number;
	zones: ReturnType<typeof useVirtualPlaybackSurfaceZones>;
}

function useVirtualPlaybackInteractions(options: InteractionOptions) {
	const [configuration, setConfiguration] =
		useState<ConfigurationState | null>(null);
	const [selectedSlots, setSelectedSlots] = useState<number[]>([]);
	const [creatingZone, setCreatingZone] = useState(false);
	const [zoneName, setZoneName] = useState("");
	const configurationArmed =
		options.state.playbackSetArmed ||
		(options.state.cueListSetArmed &&
			options.state.cueListSetTarget == null);
	const assignmentPending = options.state.cueListSetTarget != null;
	useEffect(() => {
		setConfiguration(null);
		setSelectedSlots([]);
		setCreatingZone(false);
		setZoneName("");
	}, [options.pageNumber, options.topology.ready]);

	useEffect(() => {
		const lastSlot = Math.min(
			options.rows * options.columns,
			MAX_PLAYBACK_SLOT,
		);
		setSelectedSlots((current) =>
			current.filter((slot) => slot <= lastSlot),
		);
	}, [options.rows, options.columns]);

	const openConfiguration = (
		playback: PlaybackDefinition | null,
		slot: number,
	) => {
		if (!validPlaybackSlot(slot) || options.pageNumber == null) return;
		const next =
			playback ??
			emptyConfiguration(
				options.pageNumber,
				slot,
				1,
				false,
				options.topology.cueLists[0]?.body.id ?? "",
			);
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
	};

	const assignSource = async (slot: number) => {
		const target = options.state.cueListSetTarget;
		if (!validPlaybackSlot(slot) || target == null || options.pageNumber == null)
			return;
		const source = options.playbacks.get(target);
		if (!source || source.target.type !== "cue_list") return;
		const playback = assignmentPlayback(options.pageNumber, slot, source);
		if (
			await options.topologyActions?.configureSlot(
				options.pageNumber,
				slot,
				playback,
			)
		)
			options.dispatch({ type: "SET_CUELIST_SET_ARMED", value: false });
	};

	const toggleZoneSlot = (slot: number) => {
		if (!options.zones.ready || !validPlaybackSlot(slot)) return;
		setSelectedSlots((current) =>
			current.includes(slot)
				? current.filter((candidate) => candidate !== slot)
				: [...current, slot].sort((left, right) => left - right),
		);
	};

	const createZone = async () => {
		const name = zoneName.trim();
		if (!options.zones.ready || !name || selectedSlots.length < 2) return;
		const zone: VirtualPlaybackZone = {
			id: crypto.randomUUID(),
			name,
			slots: [...selectedSlots],
		};
		if (!(await options.zones.persist([...options.zones.zones, zone]))) return;
		options.dispatch({ type: "SET_SHIFT_ARMED", value: false });
		setSelectedSlots([]);
		setZoneName("");
		setCreatingZone(false);
	};

	return {
		configuration:
			configuration?.page === options.pageNumber && options.topology.ready
				? configuration
				: null,
		setConfiguration,
		selectedSlots,
		setSelectedSlots,
		creatingZone,
		setCreatingZone,
		zoneName,
		setZoneName,
		configurationArmed,
		assignmentPending,
		openConfiguration,
		assignSource,
		toggleZoneSlot,
		createZone,
	};
}

function mappedPlaybackNumbers(
	slots: Readonly<Record<string, number>> | undefined,
	cellCount: number,
) {
	if (!slots) return [];
	return Array.from(
		{ length: Math.min(cellCount, MAX_PLAYBACK_SLOT) },
		(_, index) => slots[String(index + 1)],
	).filter((number): number is number => number != null);
}

function assignmentPlayback(
	page: number,
	slot: number,
	source: PlaybackDefinition,
) {
	const draft = emptyConfiguration(
		page,
		slot,
		1,
		false,
		source.target.type === "cue_list" ? source.target.cue_list_id : "",
	);
	return {
		...draft,
		name: source.name,
		color: source.color,
		buttons: [
			source.buttons[0] === "none" ? "go" : source.buttons[0],
			"none",
			"none",
		] as PlaybackDefinition["buttons"],
		presentation_icon: source.presentation_icon,
		presentation_image: source.presentation_image,
	};
}
