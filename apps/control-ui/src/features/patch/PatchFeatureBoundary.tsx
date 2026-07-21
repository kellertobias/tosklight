import { type PropsWithChildren, useMemo } from "react";
import { configuredServerUrl } from "../../api/LightApiClient";
import {
	browserDeskBoundaryToken,
	HttpPatchTransport,
} from "../../api/PatchTransport";
import { useServer } from "../../api/ServerContext";
import { mergeFixtureDefinitions } from "../../components/setup/fixtureProfileModel";
import {
	PatchViewProvider,
	useOptionalPatch,
	usePatchView,
} from "./PatchContext";
import { EMPTY_FIXTURES } from "./selectors";

/** Composes one lazy Patch authority for all consumers under this boundary. */
export function PatchFeatureBoundary({ children }: PropsWithChildren) {
	const existing = useOptionalPatch();
	if (existing) return children;
	return <PatchFeatureProvider>{children}</PatchFeatureProvider>;
}

function PatchFeatureProvider({ children }: PropsWithChildren) {
	const server = useServer();
	const sessionToken = server.session?.token ?? null;
	const baseUrl = configuredServerUrl();
	const deskBoundaryToken = browserDeskBoundaryToken();
	const transport = useMemo(
		() =>
			sessionToken
				? new HttpPatchTransport({
						baseUrl,
						sessionToken,
						deskBoundaryToken,
					})
				: null,
		[baseUrl, deskBoundaryToken, sessionToken],
	);
	const definitions = useMemo(
		() =>
			mergeFixtureDefinitions(server.fixtureProfiles, server.fixtureLibrary),
		[server.fixtureLibrary, server.fixtureProfiles],
	);
	return (
		<PatchViewProvider
			showId={server.bootstrap?.active_show?.id ?? null}
			initialFixtures={EMPTY_FIXTURES}
			definitions={definitions}
			transport={transport}
		>
			<PatchAuthorityActivation />
			{children}
		</PatchViewProvider>
	);
}

/**
 * Keeps the shared Patch authority hydrated for the whole desk.
 *
 * Patched fixtures are desk-wide data that many always-visible controls read, so — like the former
 * bootstrap patch load — the snapshot and stream stay active whenever a show is open, rather than
 * cold-starting on the operator's first selection. The session self-gates on an open show, so this
 * does nothing until one exists. Per-consumer rerender isolation is unaffected; it comes from the
 * scoped selectors, not from lazy activation.
 */
function PatchAuthorityActivation() {
	usePatchView();
	return null;
}
