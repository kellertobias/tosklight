import { useRef } from "react";
import { useServer } from "../../api/ServerContext";
import { requestUpdateTarget } from "../../components/control/updateWorkflow";
import type { CommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { ButtonGrid, WindowScrollArea } from "../../components/window-kit";
import { useApp } from "../../state/AppContext";
import { GroupCard } from "./GroupCard";
import type { FixtureMetadata, Group } from "./model";

export function GroupPoolGrid({
	cards,
	capabilities,
	knownFixtureIds,
	command,
	onOpenContext,
	onOpenProperties,
	onOpenRecord,
	recordGroup,
	runCommand,
}: Pick<FixtureMetadata, "capabilities" | "knownFixtureIds"> & {
	cards: (Group | null)[];
	command: CommandLineSurface;
	onOpenContext: (id: string) => void;
	onOpenProperties: (id: string) => void;
	onOpenRecord: (id: string) => void;
	recordGroup: (id: string) => Promise<unknown>;
	runCommand: (command: string) => Promise<unknown>;
}) {
	const server = useServer();
	const { state, dispatch } = useApp();
	const hold = useRef<number | null>(null);
	const cancelHold = () => {
		if (hold.current) window.clearTimeout(hold.current);
		hold.current = null;
	};
	const selectCard = (group: Group | null, index: number) => {
		const id = group?.id ?? String(index + 1);
		if (state.updateArmed) {
			requestUpdateTarget({ family: { type: "group" }, object_id: id });
			return;
		}
		if (group && /^SET\b/i.test(command.read().text.trim())) {
			onOpenProperties(group.id);
			void command.reset();
			return;
		}
		if (group && !state.storeArmed) {
			void server.selectionGesture({ type: "live_group", group_id: group.id });
			return;
		}
		if (!state.storeArmed) return;
		if (group?.body.fixtures.length) {
			onOpenRecord(group.id);
			return;
		}
		void recordGroup(id).finally(() =>
			dispatch({ type: "SET_STORE_ARMED", value: false }),
		);
	};

	return (
		<WindowScrollArea>
			<ButtonGrid className="card-pool">
				{cards.map((group, index) => (
					<GroupCard
						key={index + 1}
						group={group}
						index={index}
						knownFixtureIds={knownFixtureIds}
						capabilities={capabilities}
						selected={command.selectedGroupId === group?.id}
						storeArmed={state.storeArmed}
						updateArmed={state.updateArmed}
						beginHold={() => {
							if (group && !state.updateArmed) {
								hold.current = window.setTimeout(
									() => onOpenContext(group.id),
									600,
								);
							}
						}}
						cancelHold={cancelHold}
						openContext={() => group && onOpenContext(group.id)}
						dereference={() => group && void runCommand(`DEGRP ${group.id}`)}
						select={() => selectCard(group, index)}
					/>
				))}
			</ButtonGrid>
		</WindowScrollArea>
	);
}
