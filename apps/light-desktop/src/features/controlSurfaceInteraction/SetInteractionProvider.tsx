import {
	createContext,
	type MutableRefObject,
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

type ApplySetEvent = (
	event: SetInteractionEvent,
) => SetInteractionTerminalIntent | null;
type WriteVisibleState = (next: SetInteractionState) => Promise<boolean>;

function useInteractionScope(
	deskId: string | null,
	showId: string | null,
	surfaceId: string,
) {
	return useMemo<ControlSurfaceInteractionScope | null>(
		() => (deskId && showId ? { deskId, showId, surfaceId } : null),
		[deskId, showId, surfaceId],
	);
}

function useSetInteractionExit({
	scope,
	stateRef,
	apply,
	writeVisibleState,
}: {
	scope: ControlSurfaceInteractionScope | null;
	stateRef: MutableRefObject<SetInteractionState | null>;
	apply: ApplySetEvent;
	writeVisibleState: WriteVisibleState;
}) {
	const leave = useCallback(
		async (type: "clear" | "cancel") => {
			if (!scope || stateRef.current?.phase === "idle") return false;
			apply({ type, scope });
			if (stateRef.current) await writeVisibleState(stateRef.current);
			return true;
		},
		[apply, scope, stateRef, writeVisibleState],
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
		[apply, stateRef, writeVisibleState],
	);
	return { leave, direct };
}

function useCommandGroupRouting({
	scope,
	groupsReady,
	state,
	text,
	groups,
	chooseGroup,
	apply,
}: {
	scope: ControlSurfaceInteractionScope | null;
	groupsReady: boolean;
	state: SetInteractionState | null;
	text: string | undefined;
	groups: ReturnType<typeof usePortableGroups>;
	chooseGroup: SetInteractionController["chooseGroup"];
	apply: ApplySetEvent;
}) {
	useEffect(() => {
		if (!scope || state?.phase !== "set_armed") return;
		const commandText = text?.trim() ?? "";
		const match = commandText.match(/^ASSIGN\s+GROUP\s+(\S+)$/i);
		if (match && groupsReady) {
			const group = groups.find((candidate) => candidate.id === match[1]);
			if (group)
				void chooseGroup(
					{ objectId: group.id, objectRevision: group.revision },
					"keyboard",
				);
			return;
		}
		if (
			/^ASSIGN\s+(?:CUELIST|DYNAMIC|MACRO|TIMECODE)\s+\d+$/i.test(commandText)
		) {
			apply({
				type: "choose_object",
				source: "keyboard",
				scope,
				sourceCommand: commandText.toUpperCase(),
			});
			return;
		}
		if (commandText && !/^ASSIGN(?:\s+GROUP(?:\s+\S*)?)?$/i.test(commandText))
			apply({ type: "cancel", scope });
	}, [apply, chooseGroup, groups, groupsReady, scope, state?.phase, text]);
}

function useChoosePlayback({
	scope,
	stateRef,
	apply,
	writeVisibleState,
	topology,
	command,
}: {
	scope: ControlSurfaceInteractionScope | null;
	stateRef: MutableRefObject<SetInteractionState | null>;
	apply: ApplySetEvent;
	writeVisibleState: WriteVisibleState;
	topology: ReturnType<typeof usePlaybackTopologyActions>;
	command: ReturnType<typeof useProgrammingCommandLineActions>;
}) {
	return useCallback(
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
			if (intent?.type === "assign_object") {
				const address =
					intent.playback.addressing === "virtual"
						? `VPBK ${intent.playback.playbackNumber}`
						: `PBK ${intent.playback.addressing === "explicit_page" ? `${intent.playback.pageNumber} . ` : ""}${intent.playback.slot}`;
				await command?.execute(`${intent.sourceCommand} AT ${address}`);
			}
			if (intent && stateRef.current) await writeVisibleState(stateRef.current);
			if (intent?.type === "assign_group_master") {
				const options = {
					expectedPageRevision: intent.playback.pageObjectRevision,
					expectedPageObjectId: intent.playback.pageObjectId,
				};
				if (intent.playback.addressing === "virtual")
					await topology?.assignVirtualGroupMaster(
						intent.group.objectId,
						intent.group.objectRevision,
						intent.playback.pageNumber,
						intent.playback.playbackNumber,
						options,
					);
				else
					await topology?.assignGroupMaster(
						intent.group.objectId,
						intent.group.objectRevision,
						intent.playback.pageNumber,
						intent.playback.slot,
						{
							...options,
							expectedPlaybackRevision: intent.playback.playbackObjectRevision,
							expectedPlaybackObjectId: intent.playback.playbackObjectId,
						},
					);
			}
			if (intent?.type === "open_playback_settings")
				routeControlSurfaceIntentWithFeedback(intent);
			return intent;
		},
		[apply, command, scope, stateRef, topology, writeVisibleState],
	);
}

function useEnterSetInteraction({
	scope,
	stateRef,
	apply,
	writeVisibleState,
	text,
	groups,
	groupsReady,
}: {
	scope: ControlSurfaceInteractionScope | null;
	stateRef: MutableRefObject<SetInteractionState | null>;
	apply: ApplySetEvent;
	writeVisibleState: WriteVisibleState;
	text: string | undefined;
	groups: ReturnType<typeof usePortableGroups>;
	groupsReady: boolean;
}) {
	return useCallback(
		async (source: ControlSurfaceSource) => {
			const current = stateRef.current;
			if (!scope || !current || current.phase === "idle") return false;
			const commandText = text?.trim() ?? "";
			if (
				current.phase === "group_source_pending" &&
				commandText !== "" &&
				commandText.toUpperCase() !==
					`ASSIGN GROUP ${current.group.objectId}`.toUpperCase()
			)
				return false;
			if (current.phase === "set_armed") {
				if (commandText.toUpperCase() !== "ASSIGN") {
					const match = commandText.match(/^ASSIGN\s+GROUP\s+(\S+)$/i);
					const group =
						groupsReady && match
							? groups.find((candidate) => candidate.id === match[1])
							: null;
					if (!group) return false;
					apply({
						type: "choose_group",
						source,
						scope,
						group: { objectId: group.id, objectRevision: group.revision },
					});
				}
			}
			const intent = apply({ type: "enter", source, scope });
			if (stateRef.current) await writeVisibleState(stateRef.current);
			if (intent?.type === "open_group_settings")
				routeControlSurfaceIntentWithFeedback(intent);
			return true;
		},
		[apply, groups, groupsReady, scope, stateRef, text, writeVisibleState],
	);
}

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
	const commandRef = useRef(command);
	commandRef.current = command;
	const commandView = useProgrammingCommandLineView();
	const topology = usePlaybackTopologyActions();
	const scope = useInteractionScope(deskId, showId, surfaceId);
	const stateRef = useRef<SetInteractionState | null>(
		scope ? initialSetInteractionState(scope) : null,
	);
	const [state, setState] = useState(stateRef.current);
	useShowObjectView("group", scope !== null);
	const groups = usePortableGroups(scope !== null);
	const groupsReady = useShowObjectCollectionsReady(["group"], scope !== null);

	useEffect(() => {
		const previous = stateRef.current;
		const next = scope ? initialSetInteractionState(scope) : null;
		stateRef.current = next;
		setState(next);
		if (previous && previous.phase !== "idle") void commandRef.current?.reset();
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
				await command.replace("ASSIGN");
				return true;
			}
			if (next.phase === "group_source_pending") {
				await command.replace(`ASSIGN GROUP ${next.group.objectId}`);
				return true;
			}
			if (next.phase === "object_source_pending") {
				await command.replace(next.sourceCommand);
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

	useCommandGroupRouting({
		scope,
		groupsReady,
		state,
		text: commandView?.text,
		groups,
		chooseGroup,
		apply,
	});

	const choosePlayback = useChoosePlayback({
		scope,
		stateRef,
		apply,
		writeVisibleState,
		topology,
		command,
	});

	const enter = useEnterSetInteraction({
		scope,
		stateRef,
		apply,
		writeVisibleState,
		text: commandView?.text,
		groups,
		groupsReady,
	});

	const { leave, direct } = useSetInteractionExit({
		scope,
		stateRef,
		apply,
		writeVisibleState,
	});

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
		<SetInteractionBoundary value={value}>{children}</SetInteractionBoundary>
	);
}

function SetInteractionBoundary({
	value,
	children,
}: PropsWithChildren<{ value: SetInteractionController }>) {
	return (
		<SetInteractionContext.Provider value={value}>
			{children}
		</SetInteractionContext.Provider>
	);
}

export function useSetInteraction() {
	return useContext(SetInteractionContext);
}
