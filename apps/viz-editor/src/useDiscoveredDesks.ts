import { useEffect, useState } from "react";
import { type DeskPeer, documentSession } from "./document/session";

/**
 * The desks on the network worth loading from.
 *
 * Nothing is offered when nothing is found: a button that can only fail is worse than no button.
 * A desk that starts after this window did should still appear, and one that goes should stop
 * being offered — the browse already keeps that list, and this is only how often the bar reads it.
 */
export function useDiscoveredDesks() {
	const [desks, setDesks] = useState<DeskPeer[]>([]);
	useEffect(() => {
		let current = true;
		const look = () =>
			void documentSession
				.discoveredDesks()
				.then((found) => current && setDesks(found))
				.catch(() => current && setDesks([]));
		look();
		const timer = window.setInterval(look, 5000);
		return () => {
			current = false;
			window.clearInterval(timer);
		};
	}, []);
	return desks;
}
