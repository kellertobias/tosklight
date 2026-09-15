import { useEffect, useState } from "react";
import { documentSession, type MediaLayoutSnapshot } from "./document/session";

export const EMPTY_MEDIA_LAYOUT: MediaLayoutSnapshot = {
	fallbackAssets: [],
	servers: [],
	sources: [],
	ledModuleTypes: [],
	surfaces: [],
	projectors: [],
};

/**
 * The open show's media layout, read on mount and again whenever another window or a program using
 * the local editing API changes it — so a server an MCP tool adds appears without reopening the
 * workspace. The setter takes the snapshot an edit made here returns.
 */
export function useMediaLayout(onError: (reason: unknown) => void) {
	const [layout, setLayout] = useState(EMPTY_MEDIA_LAYOUT);
	useEffect(() => {
		let disposed = false;
		const read = () =>
			documentSession
				.mediaLayout()
				.then((next) => {
					if (!disposed) setLayout(next);
				})
				.catch(onError);
		void read();
		const listening = documentSession.onMediaLayoutChanged(() => void read());
		return () => {
			disposed = true;
			void listening.then((unlisten) => unlisten()).catch(() => undefined);
		};
	}, [onError]);
	return [layout, setLayout] as const;
}
