// Pushed telemetry.
//
// One socket, opened by whichever panel is watching and closed when it stops. Volatile state
// arrives here rather than being polled: the server decides the cadence, because the server knows
// what the analysis costs.
//
// A dropped socket is normal — a laptop lid closes, a network hiccups — so this reconnects, and it
// reports whether it is connected so a meter can say "not receiving" instead of freezing on the
// last frame it happened to get.

import { useEffect, useRef, useState } from "react";
import { telemetryUrl } from "./client";
import type { TelemetryFrame } from "./generated/media-wire";

/** How long to wait before dialling again. Long enough not to hammer a server that is restarting. */
const RECONNECT_MS = 2_000;

export interface Telemetry {
	frame: TelemetryFrame | undefined;
	connected: boolean;
}

/**
 * Subscribes while the calling component is mounted.
 *
 * `enabled` exists so a panel can stop receiving without unmounting — there is no reason to keep a
 * socket open behind a collapsed meter.
 */
export function useTelemetry(enabled = true): Telemetry {
	const [frame, setFrame] = useState<TelemetryFrame | undefined>(undefined);
	const [connected, setConnected] = useState(false);
	// Held in a ref so the reconnect timer and the effect's cleanup can reach the same socket.
	const socket = useRef<WebSocket | undefined>(undefined);

	useEffect(() => {
		if (!enabled) return;
		let closed = false;
		let timer: ReturnType<typeof setTimeout> | undefined;

		const open = () => {
			if (closed) return;
			// A browser without WebSocket, or a test environment without one, simply never
			// connects — the panel then shows what its snapshot read gave it.
			if (typeof WebSocket === "undefined") return;

			let next: WebSocket;
			try {
				next = new WebSocket(telemetryUrl());
			} catch {
				timer = setTimeout(open, RECONNECT_MS);
				return;
			}
			socket.current = next;

			next.onopen = () => setConnected(true);
			next.onmessage = (event) => {
				try {
					setFrame(JSON.parse(String(event.data)) as TelemetryFrame);
				} catch {
					// A frame this client cannot read is not worth tearing the socket down for.
				}
			};
			next.onclose = () => {
				setConnected(false);
				if (!closed) timer = setTimeout(open, RECONNECT_MS);
			};
			// `onclose` follows an error, so the reconnect is already handled there.
			next.onerror = () => setConnected(false);
		};

		open();
		return () => {
			closed = true;
			if (timer) clearTimeout(timer);
			socket.current?.close();
			socket.current = undefined;
			setConnected(false);
		};
	}, [enabled]);

	return { frame, connected };
}
