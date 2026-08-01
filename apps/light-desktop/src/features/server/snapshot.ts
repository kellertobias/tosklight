import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createServerSnapshotValue(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "dismissError"
	| "simulateError"
	| "readServerLogs"
	| "bootstrap"
	| "session"
	| "outputRoutes"
	| "patchLayers"
	| "screens"
	| "shows"
	| "matter"
	| "fixtureLibrary"
	| "fixtureProfiles"
	| "fixtureProfileWarnings"
	| "mediaServers"
	| "mediaPreviewUrls"
	| "cueObjects"
	| "deskLayout"
	| "deskLayoutScope"
	| "unresolvedMvrFixtures"
	| "commandLine"
	| "commandTargetMode"
	| "commandLinePristine"
	| "commandHistory"
	| "pendingCommandChoice"
	| "selectedFixtures"
	| "selectedGroupId"
	| "highlight"
	| "highlightError"
	| "dismissHighlightError"
> {
	const {
		api,
		setError,
		bootstrap,
		session,
		outputRoutes,
		patchLayers,
		screens,
		shows,
		matter,
		fixtureLibrary,
		fixtureProfiles,
		fixtureProfileWarnings,
		mediaServers,
		mediaPreviewUrls,
		cueObjects,
		deskLayout,
		deskLayoutScope,
		unresolvedMvrFixtures,
		commandTargetMode,
		commandLine,
		commandLinePristine,
		commandHistory,
		pendingCommandChoice,
		selectedFixtures,
		selectedGroupId,
		highlight,
		highlightError,
		setHighlightError,
		highlightErrorSticky,
	} = model;
	return {
		dismissError: () => setError(null),
		simulateError: (message) => setError(message),
		readServerLogs: (after = 0) => api.desk.auditEvents(after),
		bootstrap,
		session,
		outputRoutes,
		patchLayers,
		screens,
		shows,
		matter,
		fixtureLibrary,
		fixtureProfiles,
		fixtureProfileWarnings,
		mediaServers,
		mediaPreviewUrls,
		cueObjects,
		deskLayout,
		deskLayoutScope,
		unresolvedMvrFixtures,
		commandLine,
		commandTargetMode,
		commandLinePristine,
		commandHistory,
		pendingCommandChoice,
		selectedFixtures,
		selectedGroupId,
		highlight,
		highlightError,
		dismissHighlightError: () => {
			highlightErrorSticky.current = false;
			setHighlightError(null);
		},
	};
}
