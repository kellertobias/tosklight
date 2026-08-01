import type {
	DiscoveredPeer as WireDiscoveredPeer,
	DiscoverySnapshot as WireDiscoverySnapshot,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";

/** One other ToskLight the desk can currently see. */
export interface DiscoveredPeer {
	role: "desk" | "editor";
	name: string;
	/** The show or document it holds. `null` means it has nothing to offer. */
	show: string | null;
	address: string;
	instance: string;
}

export interface Discovery {
	/**
	 * Whether this desk is looking at all. A desk that could not open a responder answers an
	 * empty list, and the difference matters: "found nothing" and "did not look" are not the
	 * same thing to tell an operator.
	 */
	browsing: boolean;
	peers: DiscoveredPeer[];
}

/** The other ToskLights on the network, as this desk currently sees them. */
export class DiscoveryApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async peers(): Promise<Discovery> {
		const snapshot = await this.transport.request<WireDiscoverySnapshot>(
			"/api/v2/discovery/peers",
		);
		return { browsing: snapshot.browsing, peers: snapshot.peers.map(peer) };
	}
}

function peer(found: WireDiscoveredPeer): DiscoveredPeer {
	return {
		role: found.role,
		name: found.name,
		show: found.show,
		address: found.address,
		instance: found.instance,
	};
}
