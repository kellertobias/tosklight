import { useEffect, useState } from "react";
import { useServer } from "../api/ServerContext";
import {
	type RecordMode,
	RecordModeDialog,
} from "../components/shared/RecordModeDialog";
import { WindowHeader } from "../components/window-kit";
import { useApp } from "../state/AppContext";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { GroupContextMenu } from "./groupsWindow/GroupContextMenu";
import { GroupPoolGrid } from "./groupsWindow/GroupPoolGrid";
import { GroupPropertiesDialog } from "./groupsWindow/GroupPropertiesDialog";
import { useGroupPoolModel } from "./groupsWindow/model";
import type { WindowProps } from "./windowTypes";

function GroupPoolHeader() {
	const server = useServer();
	const { state, dispatch } = useApp();
	return (
		<WindowHeader
			title="Group Pool"
			info={{
				primary: `${server.selectedFixtures.length} fixtures selected`,
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
	useShowObjectView("group", active);
	const server = useServer();
	const { dispatch } = useApp();
	const model = useGroupPoolModel(server);
	const [contextGroup, setContextGroup] = useState<string | null>(null);
	const [recordGroup, setRecordGroup] = useState<string | null>(null);
	const [propertiesGroup, setPropertiesGroup] = useState<string | null>(null);
	const contextual = model.groups.find((group) => group.id === contextGroup);
	const recordTarget = model.groups.find((group) => group.id === recordGroup);
	const propertiesTarget = model.groups.find(
		(group) => group.id === propertiesGroup,
	);

	useEffect(() => {
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
	}, [model.groups]);

	const runCommand = (command: string) => server.executeCommandLine(command);
	const recordGroupCommand = async (
		id: string,
		mode: RecordMode = "overwrite",
	) => {
		const ok = await runCommand(
			mode === "merge" ? `RECORD + GROUP ${id}` : `RECORD GROUP ${id}`,
		);
		if (ok) await server.refreshGroup(id);
		return ok;
	};
	const cancelRecording = () => {
		setRecordGroup(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
	};
	const recordExistingGroup = async (mode: RecordMode) => {
		if (!recordTarget) return cancelRecording();
		await recordGroupCommand(recordTarget.id, mode);
		cancelRecording();
	};

	return (
		<div className="pool-window group-pool-window">
			{!compact && <GroupPoolHeader />}
			<GroupPoolGrid
				cards={model.cards}
				capabilities={model.capabilities}
				knownFixtureIds={model.knownFixtureIds}
				onOpenContext={setContextGroup}
				onOpenProperties={setPropertiesGroup}
				onOpenRecord={setRecordGroup}
				recordGroup={recordGroupCommand}
				runCommand={runCommand}
			/>
			{contextual && (
				<GroupContextMenu
					fixtureNames={model.fixtureNames}
					group={contextual}
					onClose={() => setContextGroup(null)}
					recordGroup={recordGroupCommand}
					runCommand={runCommand}
				/>
			)}
			{recordTarget && (
				<RecordModeDialog
					target={recordTarget.body.name ?? `Group ${recordTarget.id}`}
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
