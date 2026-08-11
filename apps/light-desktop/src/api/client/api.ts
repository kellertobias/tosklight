import { AttributeConfigurationApiClient } from "./attributeConfiguration";
import { CueThumbnailApiClient } from "./cueThumbnails";
import { DeskManagementApiClient } from "./deskManagement";
import { DiscoveryApiClient } from "./discovery";
import { DynamicsApiClient } from "./dynamics";
import { FileApiClient } from "./files";
import { FixtureApiClient } from "./fixtures";
import { HelpApiClient } from "./help";
import { MacrosApiClient } from "./macros";
import { MediaOutputApiClient } from "./mediaOutput";
import { PlaybackApiClient } from "./playback";
import { ProgrammingApiClient } from "./programming";
import { LightClientRuntime } from "./runtime";
import { SchedulesApiClient } from "./schedules";
import { SelectiveImportApiClient } from "./selectiveImport";
import { ShowObjectsApiClient } from "./showObjects";
import { ShowApiClient } from "./shows";
import { StageLayoutApiClient } from "./stageLayout";
import { TimecodesApiClient } from "./timecodes";
import { VisualizerViewApiClient } from "./visualizerView";

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
		attributes: new AttributeConfigurationApiClient(transport),
		cueThumbnails: new CueThumbnailApiClient(transport),
		desk: new DeskManagementApiClient(transport),
		discovery: new DiscoveryApiClient(transport),
		dynamics: new DynamicsApiClient(transport),
		files: new FileApiClient(transport),
		fixtures: new FixtureApiClient(transport),
		help: new HelpApiClient(transport),
		macros: new MacrosApiClient(transport),
		mediaOutput: new MediaOutputApiClient(transport),
		playback: new PlaybackApiClient(transport),
		programming: new ProgrammingApiClient(transport),
		selectiveImport: new SelectiveImportApiClient(transport),
		schedules: new SchedulesApiClient(transport),
		showObjects: new ShowObjectsApiClient(transport),
		shows: new ShowApiClient(transport),
		stageLayout: new StageLayoutApiClient(transport),
		timecodes: new TimecodesApiClient(transport),
		visualizerView: new VisualizerViewApiClient(transport),
	};
}

export type LightApi = ReturnType<typeof createLightApi>;
