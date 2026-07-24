import { bindClientMethod } from "./client/bindMethod";
import { DeskManagementApiClient } from "./client/deskManagement";
import { FileApiClient } from "./client/files";
import { FixtureApiClient } from "./client/fixtures";
import { HelpApiClient } from "./client/help";
import { MediaOutputApiClient } from "./client/mediaOutput";
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
	private readonly mediaOutputApi = new MediaOutputApiClient(this.transport);
	private readonly showApi = new ShowApiClient(this.transport);
	private readonly deskManagementApi = new DeskManagementApiClient(
		this.transport,
	);
	private readonly showObjectsApi = new ShowObjectsApiClient(this.transport);
	private readonly programmingApi = new ProgrammingApiClient(this.transport);
	private readonly playbackApi = new PlaybackApiClient(this.transport);
	private readonly helpApi = new HelpApiClient(this.transport);
	private readonly selectiveImportApi = new SelectiveImportApiClient(
		this.transport,
	);

	helpCatalog = bindClientMethod(this.helpApi, "helpCatalog");
	helpTopic = bindClientMethod(this.helpApi, "helpTopic");
	commandHistory = bindClientMethod(this.deskManagementApi, "commandHistory");
	createUser = bindClientMethod(this.deskManagementApi, "createUser");
	setDmxOverride = bindClientMethod(this.mediaOutputApi, "setDmxOverride");
	outputRuntimeLiveAction = bindClientMethod(
		this.mediaOutputApi,
		"outputRuntimeLiveAction",
	);
	highlight = bindClientMethod(this.mediaOutputApi, "highlight");
	highlightAction = bindClientMethod(this.mediaOutputApi, "highlightAction");
	setPatchPreviewHighlight = bindClientMethod(
		this.mediaOutputApi,
		"setPatchPreviewHighlight",
	);
	auditEvents = bindClientMethod(this.deskManagementApi, "auditEvents");

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

	visualization = bindClientMethod(this.mediaOutputApi, "visualization");
	dmx = bindClientMethod(this.mediaOutputApi, "dmx");
	mediaServers = bindClientMethod(this.mediaOutputApi, "mediaServers");
	refreshMediaPreview = bindClientMethod(
		this.mediaOutputApi,
		"refreshMediaPreview",
	);
	mediaPreview = bindClientMethod(this.mediaOutputApi, "mediaPreview");
	refreshMediaThumbnails = bindClientMethod(
		this.mediaOutputApi,
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

	configuration = bindClientMethod(this.deskManagementApi, "configuration");
	updateConfiguration = bindClientMethod(
		this.deskManagementApi,
		"updateConfiguration",
	);
	matterStatus = bindClientMethod(this.deskManagementApi, "matterStatus");
	speedGroup = bindClientMethod(this.deskManagementApi, "speedGroup");
	updateSpeedGroup = bindClientMethod(
		this.deskManagementApi,
		"updateSpeedGroup",
	);
	observeSpeedGroup = bindClientMethod(
		this.deskManagementApi,
		"observeSpeedGroup",
	);
	speedGroupAction = bindClientMethod(
		this.deskManagementApi,
		"speedGroupAction",
	);
	speedGroupRuntimeLiveAction = bindClientMethod(
		this.deskManagementApi,
		"speedGroupRuntimeLiveAction",
	);
	shutdown = bindClientMethod(this.deskManagementApi, "shutdown");
	deskLock = bindClientMethod(this.deskManagementApi, "deskLock");
	configureDeskLock = bindClientMethod(
		this.deskManagementApi,
		"configureDeskLock",
	);
	lockDesk = bindClientMethod(this.deskManagementApi, "lockDesk");
	unlockDesk = bindClientMethod(this.deskManagementApi, "unlockDesk");

	objects = bindClientMethod(this.showObjectsApi, "objects");
	object = bindClientMethod(this.showObjectsApi, "object");
	objectOrNull = bindClientMethod(this.showObjectsApi, "objectOrNull");
	saveOutputRoute = bindClientMethod(this.showObjectsApi, "saveOutputRoute");
	deleteOutputRoute = bindClientMethod(this.showObjectsApi, "deleteOutputRoute");
	updateUserLayout = bindClientMethod(this.showObjectsApi, "updateUserLayout");
	savePatchLayer = bindClientMethod(this.showObjectsApi, "savePatchLayer");
	recordDynamic = bindClientMethod(this.showObjectsApi, "recordDynamic");
	storePreload = bindClientMethod(this.showObjectsApi, "storePreload");
	programmers = bindClientMethod(this.deskManagementApi, "programmers");
	programmerValuesLiveAction = bindClientMethod(
		this.programmingApi,
		"programmerValuesLiveAction",
	);
	programmerPriorityLiveAction = bindClientMethod(
		this.programmingApi,
		"programmerPriorityLiveAction",
	);
	presetRecallLiveAction = bindClientMethod(
		this.programmingApi,
		"presetRecallLiveAction",
	);
	programmerPreloadLifecycleLiveAction = bindClientMethod(
		this.programmingApi,
		"programmerPreloadLifecycleLiveAction",
	);
	programmerPreloadValuesLiveAction = bindClientMethod(
		this.programmingApi,
		"programmerPreloadValuesLiveAction",
	);
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
	clearProgrammer = bindClientMethod(this.deskManagementApi, "clearProgrammer");
	selectGroup = bindClientMethod(this.programmingApi, "selectGroup");
	selectionMacro = bindClientMethod(this.programmingApi, "selectionMacro");
	align = bindClientMethod(this.programmingApi, "align");
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
	playbackRuntimeSnapshot = bindClientMethod(
		this.playbackApi,
		"playbackRuntimeSnapshot",
	);
	playbackRuntimeAction = bindClientMethod(
		this.playbackApi,
		"playbackRuntimeAction",
	);
	playbackRuntimeLiveAction = bindClientMethod(
		this.playbackApi,
		"playbackRuntimeLiveAction",
	);
	screens = bindClientMethod(this.playbackApi, "screens");
	putScreen = bindClientMethod(this.playbackApi, "putScreen");
	deleteScreen = bindClientMethod(this.playbackApi, "deleteScreen");
	setScreenPage = bindClientMethod(this.playbackApi, "setScreenPage");
	setPlaybackPage = bindClientMethod(this.playbackApi, "setPlaybackPage");
	updateControlDesk = bindClientMethod(this.playbackApi, "updateControlDesk");
	removeClient = bindClientMethod(this.playbackApi, "removeClient");
}
