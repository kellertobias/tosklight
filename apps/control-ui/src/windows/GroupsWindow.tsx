import { useEffect, useState } from "react";
import { useServer } from "../api/ServerContext";
import {
	type CommandLineSurface,
	useCommandLineSurface,
} from "../components/control/commandLine/useCommandLineSurface";
import {
	type RecordMode,
	RecordModeDialog,
} from "../components/shared/RecordModeDialog";
import { WindowHeader } from "../components/window-kit";
import { useGroupRecording } from "../features/groupRecording/GroupRecordingProvider";
import type { GroupRecordingTarget } from "../features/groupRecording/target";
import { useApp } from "../state/AppContext";
import { GroupContextMenu } from "./groupsWindow/GroupContextMenu";
import { GroupPoolGrid } from "./groupsWindow/GroupPoolGrid";
import { GroupPropertiesDialog } from "./groupsWindow/GroupPropertiesDialog";
import { useGroupPoolModel } from "./groupsWindow/model";
import type { WindowProps } from "./windowTypes";

function GroupPoolHeader({ command }: { command: CommandLineSurface }) {
	const { state, dispatch } = useApp();
	return (
		<WindowHeader
			title="Group Pool"
			info={{
				primary: `${command.selected.length} fixtures selected`,
				secondary: "Ordered selection",
			}}
			actions={[
				[
					...(state.groupsReturnToStage
						? [
								{
									id: "stage",
									label: "Back to Stage",
									onClick: () => dispatch({ type: "RETURN_TO_STAGE" }),
								},
							]
						: []),
				],
				[
					{
						id: "presets",
						label: "Presets",
						onClick: () => dispatch({ type: "OPEN_BUILTIN", kind: "presets" }),
					},
				],
			]}
		/>
	);
}

export function GroupsWindow({ active = true, compact }: WindowProps) {
	const server = useServer();
	const groupRecording = useGroupRecording();
	const command = useCommandLineSurface({
		selection: true,
		enabled: active,
		observeCommand: false,
	});
	const { dispatch } = useApp();
	const model = useGroupPoolModel(server, active);
	const [contextGroup, setContextGroup] = useState<string | null>(null);
	const [recordGroup, setRecordGroup] = useState<GroupRecordingTarget | null>(
		null,
	);
	const [propertiesGroup, setPropertiesGroup] = useState<string | null>(null);
	const contextual = model.groups.find((group) => group.id === contextGroup);
	const propertiesTarget = model.groups.find(
		(group) => group.id === propertiesGroup,
	);
	const groupScope = server.bootstrap?.active_show?.id ?? null;

	useEffect(() => {
		setContextGroup(null);
		setPropertiesGroup(null);
		setRecordGroup(null);
	}, [groupScope, model.groupRuntimeReady]);

	useEffect(() => {
		if (!active) return;
		const openRequestedGroup = (event: Event) => {
			const id = (event as CustomEvent<string>).detail;
			if (model.groups.some((group) => group.id === id)) setPropertiesGroup(id);
		};
		window.addEventListener("light:group-configuration", openRequestedGroup);
		return () =>
			window.removeEventListener(
				"light:group-configuration",
				openRequestedGroup,
			);
	}, [active, model.groups]);

	const runCommand = (value: string) => command.execute(value);
	const recordGroupAction = async (
		target: GroupRecordingTarget,
		mode: RecordMode = "overwrite",
	) => {
		if (!groupRecording) return null;
		const outcome = await groupRecording.record({
			objectId: target.objectId,
			operation: mode,
			expectedObjectRevision: target.expectedObjectRevision,
		});
		if (outcome) await command.reset();
		return outcome;
	};
	const cancelRecording = () => {
		setRecordGroup(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
	};
	const recordExistingGroup = async (mode: RecordMode) => {
		if (!recordGroup) return cancelRecording();
		await recordGroupAction(recordGroup, mode);
		cancelRecording();
	};

	return (
		<div className="pool-window group-pool-window">
			{!compact && <GroupPoolHeader command={command} />}
			{model.groupRuntimeReady ? (
				<GroupPoolGrid
					active={active}
					command={command}
					cards={model.cards}
					capabilities={model.capabilities}
					knownFixtureIds={model.knownFixtureIds}
					onOpenContext={setContextGroup}
					onOpenProperties={setPropertiesGroup}
					onOpenRecord={setRecordGroup}
					recordGroup={recordGroupAction}
					runCommand={runCommand}
				/>
			) : (
				<p className="pool-loading" role="status">
					Group runtime loading…
				</p>
			)}
			{contextual && (
				<GroupContextMenu
					fixtureNames={model.fixtureNames}
					group={contextual}
					onClose={() => setContextGroup(null)}
					recordGroup={recordGroupAction}
					runCommand={runCommand}
					setGroupMaster={model.setGroupMaster}
					canWriteMaster={model.canWriteGroupRuntime}
				/>
			)}
			{recordGroup && (
				<RecordModeDialog
					target={recordGroup.label}
					onChoose={recordExistingGroup}
					onCancel={cancelRecording}
				/>
			)}
			{propertiesTarget && (
				<GroupPropertiesDialog
					key={`${propertiesTarget.id}:${propertiesTarget.revision}`}
					group={propertiesTarget}
					onClose={() => setPropertiesGroup(null)}
				/>
			)}
		</div>
	);
}
