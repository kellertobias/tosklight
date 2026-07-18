import { createFileActions } from "../files/actions";
import { createScreenActions } from "../screens/actions";
import { createCommandLineActions } from "./commandLine";
import { createConfigurationActions } from "./configuration";
import { createCueListActions } from "./cueLists";
import { createFixtureLibraryActions } from "./fixtureLibrary";
import { createGroupDerivationActions } from "./groupDerivation";
import { createGroupEditingActions } from "./groupEditing";
import { createGroupSelectionActions } from "./groupSelection";
import { createGroupStoreActions } from "./groupStore";
import { createHighlightActions } from "./highlight";
import { createLayoutActions } from "./layouts";
import { createMediaActions } from "./media";
import type { ServerController } from "./model";
import { createMvrActions } from "./mvr";
import { createOutputActions } from "./output";
import { createPatchActions } from "./patch";
import { createPlaybackConfigurationActions } from "./playbackConfiguration";
import { createPlaybackRuntimeActions } from "./playbackRuntime";
import { createPreloadActions } from "./preload";
import { createPresetActions } from "./presets";
import { createProgrammerSelectionActions } from "./programmerSelection";
import { createProgrammerValueActions } from "./programmerValues";
import { createSessionActions } from "./session";
import { createShowLifecycleActions } from "./showLifecycle";
import { createShowRevisionActions } from "./showRevisions";
import { createServerSnapshotValue } from "./snapshot";
import { createStorePlaybackValue } from "./storePlayback";
import { createSystemActions } from "./system";
import { createUpdateActions } from "./update";

export function composeServerContextValue(model: ServerController) {
	return {
		...createServerSnapshotValue(model),
		...createFileActions(model),
		...createSessionActions(model),
		...createScreenActions(model),
		...createHighlightActions(model),
		...createUpdateActions(model),
		...createCommandLineActions(model),
		...createProgrammerSelectionActions(model),
		...createProgrammerValueActions(model),
		...createPlaybackRuntimeActions(model),
		...createPlaybackConfigurationActions(model),
		...createCueListActions(model),
		...createOutputActions(model),
		...createShowLifecycleActions(model),
		...createShowRevisionActions(model),
		...createMvrActions(model),
		...createConfigurationActions(model),
		...createLayoutActions(model),
		...createGroupSelectionActions(model),
		...createPreloadActions(model),
		...createGroupStoreActions(model),
		...createGroupEditingActions(model),
		...createGroupDerivationActions(model),
		...createPresetActions(model),
		...createSystemActions(model),
		...createMediaActions(model),
		...createFixtureLibraryActions(model),
		...createPatchActions(model),
		...createStorePlaybackValue(model),
	};
}
