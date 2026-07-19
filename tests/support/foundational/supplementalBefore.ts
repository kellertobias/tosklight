import type { FoundationalCase } from "./case";
import { commandApiBoundaries, commandVisibleUi } from "./supplementalCommand";
import {
	dimmerApiBoundaries,
	dimmerDialogsUi,
	dimmerFadeApi,
	dimmerFadeUi,
} from "./supplementalDimmers";
import {
	derivedGroupApi,
	derivedGroupUi,
	frozenGroupApi,
	frozenGroupUi,
	missingGroupApi,
	missingGroupUi,
} from "./supplementalGroups";
import { clearUi, ltpApi, presetFamilyApi } from "./supplementalProgrammer";

export const supplementalBefore: FoundationalCase[] = [
	dimmerApiBoundaries,
	dimmerFadeApi,
	dimmerFadeUi,
	commandVisibleUi,
	derivedGroupApi,
	frozenGroupApi,
	presetFamilyApi,
	missingGroupApi,
	ltpApi,
	clearUi,
	dimmerDialogsUi,
	commandApiBoundaries,
	derivedGroupUi,
	frozenGroupUi,
	missingGroupUi,
];
