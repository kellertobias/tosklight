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
import { ApiFailure, telemetryUrl } from "./client";
import type { AutomationAction, AutomationActionResult, TelemetryFrame } from "./generated/media-wire";
import { newIdentity } from "./identity";

/** How long to wait before dialling again. Long enough not to hammer a server that is restarting. */
const RECONNECT_MS = 2_000;
const liveSockets = new Set<WebSocket>();
const pendingActions = new Map<string, { socket: WebSocket; resolve: (result: AutomationActionResult) => void; reject: (error: ApiFailure) => void; timer: ReturnType<typeof setTimeout> }>();

/** A live action is sent once, only over a connected socket. Reconnect never replays it. */
export async function sendTelemetryAction(action: AutomationAction): Promise<void> {
	await sendTelemetryActionResult(action);
}

/** The acknowledged result for actions whose output snapshot is needed immediately. */
export function sendTelemetryActionResult(action: AutomationAction): Promise<AutomationActionResult> {
	const socket = [...liveSockets].find((candidate) => candidate.readyState === WebSocket.OPEN);
	if (!socket) return Promise.reject(new ApiFailure("action-disconnected", "The live connection is unavailable. Wait for reconnection and try again.", 0));
	const actionId = newIdentity();
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => {
			pendingActions.delete(actionId);
			reject(new ApiFailure("action-unconfirmed", "The server did not confirm the action. Check the current playback state before trying again.", 0));
		}, 10_000);
		pendingActions.set(actionId, { socket, resolve, reject, timer });
		try { socket.send(JSON.stringify({ ...action, actionId })); }
		catch { clearTimeout(timer); pendingActions.delete(actionId); reject(new ApiFailure("action-disconnected", "The live connection closed before the action could be sent.", 0)); }
	});
}

function failSocketActions(socket: WebSocket) {
	for (const [id, action] of pendingActions) {
		if (action.socket !== socket) continue;
		clearTimeout(action.timer);
		pendingActions.delete(id);
		action.reject(new ApiFailure("action-unconfirmed", "The live connection closed. Check the current playback state before trying again.", 0));
	}
}

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

			next.onopen = () => { liveSockets.add(next); setConnected(true); };
			next.onmessage = (event) => {
				try {
					const frame = JSON.parse(String(event.data));
					if (frame.type === "automation-action-result") {
						const action = pendingActions.get(frame.actionId);
						if (action?.socket === next) {
							clearTimeout(action.timer); pendingActions.delete(frame.actionId);
							if (frame.ok) action.resolve(frame as AutomationActionResult);
							else action.reject(new ApiFailure("action-refused", frame.message ?? "The scheduling action was refused.", 400));
						}
						return;
					}
					setFrame(frame as TelemetryFrame);
				} catch {
					// A frame this client cannot read is not worth tearing the socket down for.
				}
			};
			next.onclose = () => {
				liveSockets.delete(next);
				failSocketActions(next);
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
			if (socket.current) { liveSockets.delete(socket.current); failSocketActions(socket.current); }
			socket.current?.close();
			socket.current = undefined;
			setConnected(false);
		};
	}, [enabled]);

	return { frame, connected };
}
