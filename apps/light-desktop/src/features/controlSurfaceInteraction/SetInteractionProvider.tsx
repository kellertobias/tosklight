import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { usePlaybackTopologyActions } from "../playbackTopology/PlaybackTopologyProvider";
import {
	useProgrammingCommandLineActions,
	useProgrammingCommandLineView,
} from "../programmingInteraction/ProgrammingInteractionView";
import {
	usePortableGroups,
	useShowObjectCollectionsReady,
} from "../showObjects/ShowObjectsState";
import { useShowObjectView } from "../showObjects/ShowObjectsView";
import type {
	ControlSurfaceInteractionScope,
	ControlSurfaceSource,
	GroupInteractionIdentity,
	PlaybackInteractionIdentity,
	SetInteractionTerminalIntent,
} from "./contracts";
import { routeControlSurfaceIntentWithFeedback } from "./registry";
import {
	initialSetInteractionState,
	type SetInteractionEvent,
	type SetInteractionState,
	transitionSetInteraction,
} from "./setInteraction";

export const DESK_SET_SURFACE_ID = "desk-control-surface";

export interface SetInteractionController {
	ready: boolean;
	state: SetInteractionState | null;
	arm(source: ControlSurfaceSource): Promise<boolean>;
	chooseGroup(
		group: GroupInteractionIdentity,
		source: ControlSurfaceSource,
	): Promise<SetInteractionTerminalIntent | null>;
	choosePlayback(
		playback: PlaybackInteractionIdentity,
		source: ControlSurfaceSource,
	): Promise<SetInteractionTerminalIntent | null>;
	enter(source: ControlSurfaceSource): Promise<boolean>;
	clear(): Promise<boolean>;
	cancel(): Promise<boolean>;
	direct(
		event: Extract<
			SetInteractionEvent,
			{
				type:
					| "select_group_live"
					| "select_group_frozen"
					| "open_group_settings";
			}
		>,
	): Promise<SetInteractionTerminalIntent | null>;
}

const SetInteractionContext = createContext<SetInteractionController | null>(
	null,
);

export function SetInteractionProvider({
	children,
	deskId,
	showId,
	surfaceId = DESK_SET_SURFACE_ID,
}: PropsWithChildren<{
	deskId: string | null;
	showId: string | null;
	surfaceId?: string;
}>) {
	const command = useProgrammingCommandLineActions();
	const commandView = useProgrammingCommandLineView();
	const topology = usePlaybackTopologyActions();
	const scope = useMemo<ControlSurfaceInteractionScope | null>(
		() =>
			deskId && showId
				? {
						deskId,
						showId,
						surfaceId,
					}
				: null,
		[deskId, showId, surfaceId],
	);
	const stateRef = useRef<SetInteractionState | null>(
		scope ? initialSetInteractionState(scope) : null,
	);
	const [state, setState] = useState(stateRef.current);
	useShowObjectView("group", scope !== null);
	const groups = usePortableGroups(scope !== null);
	const groupsReady = useShowObjectCollectionsReady(["group"], scope !== null);

	useEffect(() => {
		const next = scope ? initialSetInteractionState(scope) : null;
		stateRef.current = next;
		setState(next);
	}, [scope]);

	const apply = useCallback((event: SetInteractionEvent) => {
		const current = stateRef.current;
		if (!current) return null;
		const transition = transitionSetInteraction(current, event);
		if (transition.status !== "transitioned") return null;
		stateRef.current = transition.state;
		setState(transition.state);
		return transition.intent;
	}, []);

	const writeVisibleState = useCallback(
		async (next: SetInteractionState) => {
			if (!command) return false;
			if (next.phase === "set_armed") {
				await command.replace("SET");
				return true;
			}
			if (next.phase === "group_source_pending") {
				await command.replace(`SET GROUP ${next.group.objectId}`);
				return true;
			}
			await command.reset();
			return true;
		},
		[command],
	);

	const arm = useCallback(
		async (source: ControlSurfaceSource) => {
			if (!scope || !command) return false;
			const current = stateRef.current;
			if (!current) return false;
			const transition = transitionSetInteraction(current, {
				type: "arm_set",
				source,
				scope,
			});
			if (transition.status !== "transitioned") return false;
			stateRef.current = transition.state;
			setState(transition.state);
			return writeVisibleState(transition.state);
		},
		[command, scope, writeVisibleState],
	);

	const chooseGroup = useCallback(
		async (group: GroupInteractionIdentity, source: ControlSurfaceSource) => {
			if (!scope) return null;
			const intent = apply({ type: "choose_group", group, source, scope });
			if (stateRef.current) await writeVisibleState(stateRef.current);
			return intent;
		},
		[apply, scope, writeVisibleState],
	);

	useEffect(() => {
		if (!scope || !groupsReady || state?.phase !== "set_armed") return;
		const text = commandView?.text.trim() ?? "";
		const match = text.match(/^SET\s+GROUP\s+(\S+)$/i);
		if (match) {
			const group = groups.find((candidate) => candidate.id === match[1]);
			if (group)
				void chooseGroup(
					{ objectId: group.id, objectRevision: group.revision },
					"keyboard",
				);
			return;
		}
		if (text && !/^SET(?:\s+GROUP(?:\s+\S*)?)?$/i.test(text))
			apply({ type: "cancel", scope });
	}, [
		apply,
		chooseGroup,
		commandView?.text,
		groups,
		groupsReady,
		scope,
		state?.phase,
	]);

	const choosePlayback = useCallback(
		async (
			playback: PlaybackInteractionIdentity,
			source: ControlSurfaceSource,
		) => {
			if (!scope) return null;
			const intent = apply({
				type: "choose_playback",
				playback,
				source,
				scope,
			});
			if (intent && stateRef.current) await writeVisibleState(stateRef.current);
			if (intent?.type === "assign_group_master")
				if (intent.playback.addressing === "virtual")
					await topology?.assignVirtualGroupMaster(
						intent.group.objectId,
						intent.group.objectRevision,
						intent.playback.pageNumber,
						intent.playback.playbackNumber,
						{
							expectedPageRevision: intent.playback.pageObjectRevision,
							expectedPageObjectId: intent.playback.pageObjectId,
						},
					);
				else
					await topology?.assignGroupMaster(
						intent.group.objectId,
						intent.group.objectRevision,
						intent.playback.pageNumber,
						intent.playback.slot,
						{
							expectedPageRevision: intent.playback.pageObjectRevision,
							expectedPageObjectId: intent.playback.pageObjectId,
							expectedPlaybackRevision: intent.playback.playbackObjectRevision,
							expectedPlaybackObjectId: intent.playback.playbackObjectId,
						},
					);
			if (intent?.type === "open_playback_settings")
				routeControlSurfaceIntentWithFeedback(intent);
			return intent;
		},
		[apply, scope, topology, writeVisibleState],
	);

	const enter = useCallback(
		async (source: ControlSurfaceSource) => {
			if (!scope || stateRef.current?.phase === "idle") return false;
			if (stateRef.current?.phase === "set_armed") {
				const text = commandView?.text.trim() ?? "";
				if (text.toUpperCase() !== "SET") {
					const match = text.match(/^SET\s+GROUP\s+(\S+)$/i);
					const group =
						groupsReady && match
							? groups.find((candidate) => candidate.id === match[1])
							: null;
					if (!group) return false;
					apply({
						type: "choose_group",
						source,
						scope,
						group: {
							objectId: group.id,
							objectRevision: group.revision,
						},
					});
				}
			}
			const intent = apply({ type: "enter", source, scope });
			if (stateRef.current) await writeVisibleState(stateRef.current);
			if (intent?.type === "open_group_settings")
				routeControlSurfaceIntentWithFeedback(intent);
			return true;
		},
		[apply, commandView?.text, groups, groupsReady, scope, writeVisibleState],
	);

	const leave = useCallback(
		async (type: "clear" | "cancel") => {
			if (!scope || stateRef.current?.phase === "idle") return false;
			apply({ type, scope });
			if (stateRef.current) await writeVisibleState(stateRef.current);
			return true;
		},
		[apply, scope, writeVisibleState],
	);

	const direct = useCallback(
		async (
			event: Extract<
				SetInteractionEvent,
				{
					type:
						| "select_group_live"
						| "select_group_frozen"
						| "open_group_settings";
				}
			>,
		) => {
			const replacesPending = stateRef.current?.phase !== "idle";
			const intent = apply(event);
			if (replacesPending && stateRef.current)
				await writeVisibleState(stateRef.current);
			if (intent?.type === "open_group_settings")
				routeControlSurfaceIntentWithFeedback(intent);
			return intent;
		},
		[apply, writeVisibleState],
	);

	const value = useMemo<SetInteractionController>(
		() => ({
			ready: scope !== null && command !== null,
			state,
			arm,
			chooseGroup,
			choosePlayback,
			enter,
			clear: () => leave("clear"),
			cancel: () => leave("cancel"),
			direct,
		}),
		[
			scope,
			command,
			state,
			arm,
			chooseGroup,
			choosePlayback,
			enter,
			leave,
			direct,
		],
	);
	return (
		<SetInteractionContext.Provider value={value}>
			{children}
		</SetInteractionContext.Provider>
	);
}

export function useSetInteraction() {
	return useContext(SetInteractionContext);
}
