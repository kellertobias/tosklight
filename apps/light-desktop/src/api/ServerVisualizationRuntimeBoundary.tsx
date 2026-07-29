import { type PropsWithChildren, useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import type { SessionRole } from "../features/session/ownership";
import { VisualizationRuntimeProvider } from "../features/visualizationRuntime/VisualizationRuntimeView";
import { configuredServerUrl } from "./client/serverLocation";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpVisualizationRuntimeTransport } from "./VisualizationRuntimeTransport";

/** Supplies the exact active Show/session/server scope without broad UI context reads. */
export function ServerVisualizationRuntimeBoundary({
	children,
	state,
	sessionRole,
}: PropsWithChildren<{ state: ServerState; sessionRole: SessionRole }>) {
	const showId = state.bootstrap?.active_show?.id ?? null;
	const sessionId = state.session?.session_id ?? null;
	const sessionToken = state.session?.token ?? null;
	const serverUrl = configuredServerUrl();
	const deskBoundaryToken = safeDeskBoundaryToken();
	const authorityKey = [
		serverUrl,
		state.connectionGeneration,
		sessionId ?? "",
		state.session?.client_id ?? "",
		state.session?.desk.id ?? "",
		state.session?.user.id ?? "",
	].join("|");
	const desktopAuthorityKey = [
		serverUrl,
		sessionId ?? "",
		state.session?.desk.id ?? "",
		state.session?.user.id ?? "",
	].join("|");
	const transport = useMemo(
		() =>
			showId && sessionId && sessionToken
				? new HttpVisualizationRuntimeTransport({
						baseUrl: serverUrl,
						sessionToken,
						showId,
						sessionId,
						authorityKey,
						deskBoundaryToken,
					})
				: null,
		[
			authorityKey,
			deskBoundaryToken,
			serverUrl,
			sessionId,
			sessionToken,
			showId,
		],
	);
	return (
		<VisualizationRuntimeProvider
			showId={showId}
			sessionId={sessionId}
			authorityKey={authorityKey}
			transport={transport}
			desktopRole={sessionRole === "primary" ? "owner" : "mirror"}
			desktopAuthorityKey={desktopAuthorityKey}
		>
			{children}
		</VisualizationRuntimeProvider>
	);
}

function safeDeskBoundaryToken() {
	if (
		typeof globalThis.sessionStorage?.getItem !== "function" ||
		typeof globalThis.localStorage?.getItem !== "function"
	)
		return "";
	return browserDeskBoundaryToken();
}
