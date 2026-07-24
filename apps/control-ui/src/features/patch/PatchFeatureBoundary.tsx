import { type PropsWithChildren, useMemo } from "react";
import { configuredServerUrl } from "../../api/client/serverLocation";
import {
	browserDeskBoundaryToken,
	HttpPatchTransport,
} from "../../api/PatchTransport";
import {
	useActiveShowId,
	useSessionSnapshot,
} from "../deskSnapshot/DeskSnapshotState";
import { useFixtureLibrary } from "../fixtureLibrary/FixtureLibraryContext";
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
	const library = useFixtureLibrary();
const session = useSessionSnapshot();
const activeShowId = useActiveShowId();
	const sessionToken = session?.token ?? null;
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
			mergeFixtureDefinitions(
				library?.fixtureProfiles ?? [],
				library?.fixtureLibrary ?? [],
			),
		[library?.fixtureLibrary, library?.fixtureProfiles],
	);
	return (
		<PatchViewProvider
			showId={activeShowId}
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
