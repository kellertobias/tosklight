import { type PropsWithChildren, useMemo } from "react";
import { configuredServerUrl } from "../../api/LightApiClient";
import {
	browserDeskBoundaryToken,
	HttpPatchTransport,
} from "../../api/PatchTransport";
import { useServer } from "../../api/ServerContext";
import { mergeFixtureDefinitions } from "../../components/setup/fixtureProfileModel";
import { PatchViewProvider, useOptionalPatch } from "./PatchContext";

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
			initialFixtures={server.patch?.fixtures ?? []}
			definitions={definitions}
			transport={transport}
		>
			{children}
		</PatchViewProvider>
	);
}
