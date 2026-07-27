import { DeskManagementApiClient } from "./deskManagement";
import { DynamicsApiClient } from "./dynamics";
import { FileApiClient } from "./files";
import { FixtureApiClient } from "./fixtures";
import { HelpApiClient } from "./help";
import { MediaOutputApiClient } from "./mediaOutput";
import { PlaybackApiClient } from "./playback";
import { ProgrammingApiClient } from "./programming";
import { LightClientRuntime } from "./runtime";
import { SelectiveImportApiClient } from "./selectiveImport";
import { ShowObjectsApiClient } from "./showObjects";
import { ShowApiClient } from "./shows";

/**
 * Capability registry for one desk connection.
 *
 * The runtime owns authentication, the live event socket, and the shared
 * transport. HTTP capabilities stay independently typed instead of being
 * flattened into another compatibility client.
 */
export function createLightApi(baseUrl?: string) {
	const runtime = new LightClientRuntime(baseUrl);
	const transport = runtime.capabilityTransport();
	return {
		runtime,
		desk: new DeskManagementApiClient(transport),
		dynamics: new DynamicsApiClient(transport),
		files: new FileApiClient(transport),
		fixtures: new FixtureApiClient(transport),
		help: new HelpApiClient(transport),
		mediaOutput: new MediaOutputApiClient(transport),
		playback: new PlaybackApiClient(transport),
		programming: new ProgrammingApiClient(transport),
		selectiveImport: new SelectiveImportApiClient(transport),
		showObjects: new ShowObjectsApiClient(transport),
		shows: new ShowApiClient(transport),
	};
}

export type LightApi = ReturnType<typeof createLightApi>;
