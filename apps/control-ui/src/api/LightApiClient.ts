import { bindClientMethod } from "./client/bindMethod";
import { ConfigurationApiClient } from "./client/configuration";
import { DeskApiClient } from "./client/desk";
import { FileApiClient } from "./client/files";
import { FixtureApiClient } from "./client/fixtures";
import { HelpApiClient } from "./client/help";
import { MediaApiClient } from "./client/media";
import { OutputApiClient } from "./client/output";
import { PlaybackApiClient } from "./client/playback";
import { ProgrammingApiClient } from "./client/programming";
import { LightClientRuntime } from "./client/runtime";
import { SelectiveImportApiClient } from "./client/selectiveImport";
import { ShowObjectsApiClient } from "./client/showObjects";
import { ShowApiClient } from "./client/shows";

export {
	configuredServerUrl,
	defaultServerUrl,
	saveServerUrl,
} from "./client/serverLocation";

/**
 * Flat compatibility facade over capability-focused API clients.
 *
 * The bound properties deliberately preserve the established `client.method()`
 * surface while each capability owns its paths, payloads, and response types.
 */
export class LightApiClient extends LightClientRuntime {
	private readonly fileApi = new FileApiClient(this.transport);
	private readonly fixtureApi = new FixtureApiClient(this.transport);
	private readonly mediaApi = new MediaApiClient(this.transport);
	private readonly showApi = new ShowApiClient(this.transport);
	private readonly configurationApi = new ConfigurationApiClient(
		this.transport,
	);
	private readonly showObjectsApi = new ShowObjectsApiClient(this.transport);
	private readonly programmingApi = new ProgrammingApiClient(this.transport);
	private readonly playbackApi = new PlaybackApiClient(this.transport);
	private readonly helpApi = new HelpApiClient(this.transport);
	private readonly deskApi = new DeskApiClient(this.transport);
	private readonly outputApi = new OutputApiClient(this.transport);
	private readonly selectiveImportApi = new SelectiveImportApiClient(
		this.transport,
	);

	helpCatalog = bindClientMethod(this.helpApi, "helpCatalog");
	helpTopic = bindClientMethod(this.helpApi, "helpTopic");
	commandHistory = bindClientMethod(this.deskApi, "commandHistory");
	createUser = bindClientMethod(this.deskApi, "createUser");
	setDmxOverride = bindClientMethod(this.outputApi, "setDmxOverride");
	highlight = bindClientMethod(this.outputApi, "highlight");
	highlightAction = bindClientMethod(this.outputApi, "highlightAction");
	setPatchPreviewHighlight = bindClientMethod(
		this.outputApi,
		"setPatchPreviewHighlight",
	);
	auditEvents = bindClientMethod(this.deskApi, "auditEvents");
	setMaster = bindClientMethod(this.outputApi, "setMaster");

	fileRoots = bindClientMethod(this.fileApi, "fileRoots");
	fileEntries = bindClientMethod(this.fileApi, "fileEntries");
	fileMetadata = bindClientMethod(this.fileApi, "fileMetadata");
	readFileNote = bindClientMethod(this.fileApi, "readFileNote");
	saveFileNote = bindClientMethod(this.fileApi, "saveFileNote");
	readTextFile = bindClientMethod(this.fileApi, "readTextFile");
	saveTextFile = bindClientMethod(this.fileApi, "saveTextFile");
	fileOperation = bindClientMethod(this.fileApi, "fileOperation");
	fileContent = bindClientMethod(this.fileApi, "fileContent");
	fileStreamUrl = bindClientMethod(this.fileApi, "fileStreamUrl");
	fileThumbnail = bindClientMethod(this.fileApi, "fileThumbnail");
	claimFileInput = bindClientMethod(this.fileApi, "claimFileInput");
	releaseFileInput = bindClientMethod(this.fileApi, "releaseFileInput");

	patch = bindClientMethod(this.fixtureApi, "patch");
	fixtureLibrary = bindClientMethod(this.fixtureApi, "fixtureLibrary");
	fixtureProfiles = bindClientMethod(this.fixtureApi, "fixtureProfiles");
	fixtureProfileWarnings = bindClientMethod(
		this.fixtureApi,
		"fixtureProfileWarnings",
	);
	fixtureProfileRevisions = bindClientMethod(
		this.fixtureApi,
		"fixtureProfileRevisions",
	);
	putFixtureProfile = bindClientMethod(this.fixtureApi, "putFixtureProfile");
	deleteFixtureProfile = bindClientMethod(
		this.fixtureApi,
		"deleteFixtureProfile",
	);
	putFixtureProfileSourceGdtf = bindClientMethod(
		this.fixtureApi,
		"putFixtureProfileSourceGdtf",
	);
	importFixturePackage = bindClientMethod(
		this.fixtureApi,
		"importFixturePackage",
	);
	exportFixturePackage = bindClientMethod(
		this.fixtureApi,
		"exportFixturePackage",
	);
	putFixtureDefinition = bindClientMethod(
		this.fixtureApi,
		"putFixtureDefinition",
	);
	deleteFixtureDefinition = bindClientMethod(
		this.fixtureApi,
		"deleteFixtureDefinition",
	);

	visualization = bindClientMethod(this.mediaApi, "visualization");
	dmx = bindClientMethod(this.mediaApi, "dmx");
	mediaServers = bindClientMethod(this.mediaApi, "mediaServers");
	refreshMediaPreview = bindClientMethod(this.mediaApi, "refreshMediaPreview");
	mediaPreview = bindClientMethod(this.mediaApi, "mediaPreview");
	refreshMediaThumbnails = bindClientMethod(
		this.mediaApi,
		"refreshMediaThumbnails",
	);

	shows = bindClientMethod(this.showApi, "shows");
	createShow = bindClientMethod(this.showApi, "createShow");
	openShow = bindClientMethod(this.showApi, "openShow");
	openCleanDefaultShow = bindClientMethod(this.showApi, "openCleanDefaultShow");
	renameShow = bindClientMethod(this.showApi, "renameShow");
	overwriteShow = bindClientMethod(this.showApi, "overwriteShow");
	showRevisions = bindClientMethod(this.showApi, "showRevisions");
	saveShowRevision = bindClientMethod(this.showApi, "saveShowRevision");
	openShowRevision = bindClientMethod(this.showApi, "openShowRevision");
	rollbackShow = bindClientMethod(this.showApi, "rollbackShow");
	downloadShow = bindClientMethod(this.showApi, "downloadShow");
	previewMvr = bindClientMethod(this.showApi, "previewMvr");
	applyMvr = bindClientMethod(this.showApi, "applyMvr");
	mvrExportPreview = bindClientMethod(this.showApi, "mvrExportPreview");
	downloadMvr = bindClientMethod(this.showApi, "downloadMvr");
	selectiveImportCatalog = bindClientMethod(this.selectiveImportApi, "catalog");
	previewSelectiveImport = bindClientMethod(this.selectiveImportApi, "preview");
	applySelectiveImport = bindClientMethod(this.selectiveImportApi, "apply");

	configuration = bindClientMethod(this.configurationApi, "configuration");
	updateConfiguration = bindClientMethod(
		this.configurationApi,
		"updateConfiguration",
	);
	matterStatus = bindClientMethod(this.configurationApi, "matterStatus");
	speedGroup = bindClientMethod(this.configurationApi, "speedGroup");
	updateSpeedGroup = bindClientMethod(
		this.configurationApi,
		"updateSpeedGroup",
	);
	observeSpeedGroup = bindClientMethod(
		this.configurationApi,
		"observeSpeedGroup",
	);
	speedGroupAction = bindClientMethod(
		this.configurationApi,
		"speedGroupAction",
	);
	shutdown = bindClientMethod(this.configurationApi, "shutdown");
	deskLock = bindClientMethod(this.configurationApi, "deskLock");
	configureDeskLock = bindClientMethod(
		this.configurationApi,
		"configureDeskLock",
	);
	lockDesk = bindClientMethod(this.configurationApi, "lockDesk");
	unlockDesk = bindClientMethod(this.configurationApi, "unlockDesk");

	objects = bindClientMethod(this.showObjectsApi, "objects");
	object = bindClientMethod(this.showObjectsApi, "object");
	objectOrNull = bindClientMethod(this.showObjectsApi, "objectOrNull");
	putObject = bindClientMethod(this.showObjectsApi, "putObject");
	deleteObject = bindClientMethod(this.showObjectsApi, "deleteObject");
	storePreload = bindClientMethod(this.showObjectsApi, "storePreload");
	undoObject = bindClientMethod(this.showObjectsApi, "undoObject");
	programmers = bindClientMethod(this.programmingApi, "programmers");
	programmingInteractionSnapshot = bindClientMethod(
		this.programmingApi,
		"programmingInteractionSnapshot",
	);
	replaceProgrammingCommandLine = bindClientMethod(
		this.programmingApi,
		"replaceProgrammingCommandLine",
	);
	applyProgrammingSelection = bindClientMethod(
		this.programmingApi,
		"applyProgrammingSelection",
	);
	clearProgrammer = bindClientMethod(this.programmingApi, "clearProgrammer");
	selectGroup = bindClientMethod(this.programmingApi, "selectGroup");
	selectionMacro = bindClientMethod(this.programmingApi, "selectionMacro");
	align = bindClientMethod(this.programmingApi, "align");
	preload = bindClientMethod(this.programmingApi, "preload");
	controlFixtureAction = bindClientMethod(
		this.programmingApi,
		"controlFixtureAction",
	);
	generateFixturePresets = bindClientMethod(
		this.programmingApi,
		"generateFixturePresets",
	);
	setGroupMaster = bindClientMethod(this.programmingApi, "setGroupMaster");
	setGroupMasterFlash = bindClientMethod(
		this.programmingApi,
		"setGroupMasterFlash",
	);
	setSelection = bindClientMethod(this.programmingApi, "setSelection");
	selectionGesture = bindClientMethod(this.programmingApi, "selectionGesture");
	setCommandLine = bindClientMethod(this.programmingApi, "setCommandLine");
	setCommandTarget = bindClientMethod(this.programmingApi, "setCommandTarget");
	executeCommandLine = bindClientMethod(
		this.programmingApi,
		"executeCommandLine",
	);
	undoProgrammer = bindClientMethod(this.programmingApi, "undoProgrammer");
	applyPreset = bindClientMethod(this.programmingApi, "applyPreset");

	playbacks = bindClientMethod(this.playbackApi, "playbacks");
	playbackRuntimeSnapshot = bindClientMethod(
		this.playbackApi,
		"playbackRuntimeSnapshot",
	);
	playbackRuntimeAction = bindClientMethod(
		this.playbackApi,
		"playbackRuntimeAction",
	);
	screens = bindClientMethod(this.playbackApi, "screens");
	putScreen = bindClientMethod(this.playbackApi, "putScreen");
	deleteScreen = bindClientMethod(this.playbackApi, "deleteScreen");
	setScreenPage = bindClientMethod(this.playbackApi, "setScreenPage");
	playbackAction = bindClientMethod(this.playbackApi, "playbackAction");
	poolPlaybackAction = bindClientMethod(this.playbackApi, "poolPlaybackAction");
	virtualPlaybackExclusionZones = bindClientMethod(
		this.playbackApi,
		"virtualPlaybackExclusionZones",
	);
	saveVirtualPlaybackExclusionZones = bindClientMethod(
		this.playbackApi,
		"saveVirtualPlaybackExclusionZones",
	);
	savePlaybackSlot = bindClientMethod(this.playbackApi, "savePlaybackSlot");
	clearPlaybackSlot = bindClientMethod(this.playbackApi, "clearPlaybackSlot");
	setPlaybackPage = bindClientMethod(this.playbackApi, "setPlaybackPage");
	updateControlDesk = bindClientMethod(this.playbackApi, "updateControlDesk");
	removeClient = bindClientMethod(this.playbackApi, "removeClient");
}
