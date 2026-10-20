import { PoolGrid, PoolTileLayoutProvider, type PoolSlotViewModel } from "@tosklight/ui/pools";
import type { MediaPaneServer } from "./mediaPaneModel";

/** Server navigation retains its single-column layout independently of the file tile grid. */
export function MediaServerShortcuts({ servers, selectedServerId, onSelectServer }: {
	servers: MediaPaneServer[];
	selectedServerId: string;
	onSelectServer(id: string): void;
}) {
	return <section className="media-server-shortcuts" aria-label="Media servers">
		<PoolTileLayoutProvider>
			<PoolGrid
				className="media-server-pool"
				columns={1}
				fillEmptySlots={false}
				minimumCardWidth={100}
				slots={mediaServerPoolSlots(servers, selectedServerId)}
				emptySlot={(position) => ({
					id: `empty-${position}`, position,
					card: { number: position + 1, primary: "Empty", states: ["empty"] },
				})}
				onSlotClick={(serverId) => {
					if (servers.some((server) => server.id === serverId)) onSelectServer(serverId);
				}}
			/>
		</PoolTileLayoutProvider>
	</section>;
}

function mediaServerPoolSlots(
	patchedServers: MediaPaneServer[],
	selectedServerId: string,
): PoolSlotViewModel<string>[] {
	return [
		...patchedServers.map<PoolSlotViewModel<string>>((server, position) => ({
			id: server.id,
			position,
			card: {
				number: server.fixtureLabel ?? "—",
				primary: server.name,
				kind: "preset" as const,
				states: server.id === selectedServerId ? ["selected"] : [],
			},
		})),
		...Array.from(
			{ length: Math.max(0, 6 - patchedServers.length) },
			(_, emptyIndex): PoolSlotViewModel<string> => {
				const position = patchedServers.length + emptyIndex;
				return {
					id: `empty-server-${position}`,
					position,
					card: {
						number: position + 1,
						primary: "Empty",
						kind: "preset" as const,
						states: ["empty", "disabled"],
					},
				};
			},
		),
	];
}

