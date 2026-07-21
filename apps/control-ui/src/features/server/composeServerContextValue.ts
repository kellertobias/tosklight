import { createFileActions } from "../files/actions";
import { createScreenActions } from "../screens/actions";
import { createCommandLineActions } from "./commandLine";
import { createConfigurationActions } from "./configuration";
import { createFixtureLibraryActions } from "./fixtureLibrary";
import { createFixtureProgrammingActions } from "./fixtureProgramming";
import { createGroupSelectionActions } from "./groupSelection";
import { createHighlightActions } from "./highlight";
import { createLayoutActions } from "./layouts";
import { createMediaActions } from "./media";
import type { ServerController } from "./model";
import { createMvrActions } from "./mvr";
import { createOutputActions } from "./output";
import { createPatchActions } from "./patch";
import { createPreloadActions } from "./preload";
import { createProgrammerSelectionActions } from "./programmerSelection";
import { createSessionActions } from "./session";
import { createShowLifecycleActions } from "./showLifecycle";
import { createShowRevisionActions } from "./showRevisions";
import { createServerSnapshotValue } from "./snapshot";
import { createSystemActions } from "./system";

export function composeServerContextValue(model: ServerController) {
	return {
		...createServerSnapshotValue(model),
		...createFileActions(model),
		...createSessionActions(model),
		...createScreenActions(model),
		...createHighlightActions(model),
		...createCommandLineActions(model),
		...createProgrammerSelectionActions(model),
		...createFixtureProgrammingActions(model),
		...createOutputActions(model),
		...createShowLifecycleActions(model),
		...createShowRevisionActions(model),
		...createMvrActions(model),
		...createConfigurationActions(model),
		...createLayoutActions(model),
		...createGroupSelectionActions(model),
		...createPreloadActions(model),
		...createSystemActions(model),
		...createMediaActions(model),
		...createFixtureLibraryActions(model),
		...createPatchActions(model),
	};
}
