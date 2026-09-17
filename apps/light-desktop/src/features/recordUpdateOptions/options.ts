import type {
	RecordUpdateOption,
	UpdateMode,
	UpdateSettings,
	UpdateTargetRequest,
} from "../../api/types";
import type { CueRecordOperation } from "../cueRecording/contracts";

export type RecordUpdateVerb = "RECORD" | "UPDATE";

export interface RecordUpdateOptionDefinition {
	value: RecordUpdateOption;
	label: string;
	/** The words the command line shows after RECORD or UPDATE for a one-off choice. */
	keyword: string;
	record: string;
	update: string;
}

export const RECORD_UPDATE_OPTIONS: readonly RecordUpdateOptionDefinition[] = [
	{
		value: "smart",
		label: "Smart",
		keyword: "SMART",
		record:
			"The regular behaviour. A Cuelist with one Cue asks whether to add, merge, or overwrite; otherwise a new Cue is added.",
		update:
			"The regular behaviour. A touched target opens the Update preview with its configured mode; a command uses Update.",
	},
	{
		value: "merge",
		label: "Merge",
		keyword: "MERGE",
		record:
			"Merges the programmer into the Cue the playback is on, or the only Cue of a stopped Cuelist. Programmer values win.",
		update:
			"Puts every programmer value into the current Cue (Update All). Presets and Groups also gain new fixtures.",
	},
	{
		value: "add_existing",
		label: "Add Existing",
		keyword: "ADD EXISTING",
		record:
			"Adds only what that Cue does not store yet. Values the Cue already has never change.",
		update:
			"Adds values for fixture attributes the Cuelist already knows (Update Known). Presets and Groups change only what they store.",
	},
	{
		value: "add_cue",
		label: "Add Cue",
		keyword: "ADD CUE",
		record: "Always stores the programmer as a new Cue at the end.",
		update:
			"Stores the programmer as a new Cue at the end of the touched Cuelist. Presets and Groups update as Smart.",
	},
];

export function optionLabel(option: RecordUpdateOption) {
	return (
		RECORD_UPDATE_OPTIONS.find((candidate) => candidate.value === option)
			?.label ?? option
	);
}

function optionPattern(verb: RecordUpdateVerb) {
	return new RegExp(
		`^\\s*${verb}(?:\\s+(SMART|MERGE|ADD\\s+EXISTING|ADD\\s+CUE))?(?=\\s|$)\\s*`,
		"i",
	);
}

/** The one-off option an armed RECORD or UPDATE line names, if any. */
export function commandLineOption(
	text: string,
	verb: RecordUpdateVerb,
): RecordUpdateOption | null {
	const match = optionPattern(verb).exec(text);
	if (!match?.[1]) return null;
	const keyword = match[1].toUpperCase().replace(/\s+/g, " ");
	return (
		RECORD_UPDATE_OPTIONS.find((option) => option.keyword === keyword)
			?.value ?? null
	);
}

/**
 * The armed command line after a choice. A choice equal to the stored default needs no word,
 * because a plain RECORD or UPDATE already uses it; whatever followed the verb is kept.
 */
export function armedCommandLine(
	current: string,
	verb: RecordUpdateVerb,
	option: RecordUpdateOption,
	storedDefault: RecordUpdateOption,
) {
	const match = optionPattern(verb).exec(current);
	const rest = match ? current.slice(match[0].length) : "";
	const keyword =
		option === storedDefault
			? ""
			: `${RECORD_UPDATE_OPTIONS.find((candidate) => candidate.value === option)?.keyword} `;
	return `${verb} ${keyword}${rest}`;
}

/** The effective option for this Record or Update: the one-off choice, else the stored default. */
export function effectiveOption(
	text: string,
	verb: RecordUpdateVerb,
	storedDefault: RecordUpdateOption,
) {
	return commandLineOption(text, verb) ?? storedDefault;
}

export type TouchCueRecordChoice = "add" | "merge" | "overwrite";

export interface CueRecordPlan {
	operation: CueRecordOperation;
	cueNumber?: string;
}

/**
 * What a non-Smart option records on a touched Cuelist or playback, without a question. The
 * server resolves the playback's active Cue, or the only Cue of a stopped Cuelist. `null` is
 * Smart, which keeps today's question for one-Cue lists.
 */
export function optionRecordPlan(
	option: RecordUpdateOption,
): CueRecordPlan | null {
	if (option === "merge") return { operation: "merge" };
	if (option === "add_existing") return { operation: "add_missing" };
	if (option === "add_cue") return { operation: "add_cue" };
	return null;
}

/** Smart's Add / Merge / Overwrite answer for a one-Cue list (Add for any other list). */
export function smartChoiceRecordPlan(
	choice: TouchCueRecordChoice,
	onlyCueNumber: string | undefined,
): CueRecordPlan {
	const cueNumber = choice === "add" ? undefined : onlyCueNumber;
	return {
		operation: choice === "merge" ? "merge" : "overwrite",
		...(cueNumber ? { cueNumber } : {}),
	};
}

/** How a touched Cuelist or playback is recorded, asking only where Smart asks. */
export async function touchCueRecordPlan(
	option: RecordUpdateOption,
	cueNumbers: string[],
	ask: (cueNumbers: string[]) => Promise<TouchCueRecordChoice | null>,
): Promise<CueRecordPlan | null> {
	const planned = optionRecordPlan(option);
	if (planned) return planned;
	const choice = await ask(cueNumbers);
	return choice ? smartChoiceRecordPlan(choice, cueNumbers[0]) : null;
}

/** The option a touch Record uses: the one-off choice on the line, else the desk default. */
export async function resolveRecordOption(
	text: string,
	update: { loadSettings(): Promise<UpdateSettings | null> } | null,
): Promise<RecordUpdateOption> {
	const named = commandLineOption(text, "RECORD");
	if (named) return named;
	try {
		return (await update?.loadSettings())?.record_default ?? "smart";
	} catch {
		return "smart";
	}
}

/**
 * Records a touched Cuelist or playback: a non-Smart option records directly; Smart asks for a
 * one-Cue list and otherwise adds a Cue, exactly as before.
 */
export async function recordTouchedTarget({
	commandText,
	update,
	cueNumbers,
	record,
	ask,
}: {
	commandText: string;
	update: { loadSettings(): Promise<UpdateSettings | null> } | null;
	cueNumbers: string[];
	record: (plan: CueRecordPlan) => unknown;
	ask: (onlyCueNumber: string) => void;
}) {
	const option = await resolveRecordOption(commandText, update);
	const planned = optionRecordPlan(option);
	if (planned) await record(planned);
	else if (cueNumbers.length === 1) ask(cueNumbers[0]);
	else await record(smartChoiceRecordPlan("add", undefined));
}

/** The Update mode an option stands for on a touched target; `null` means record a new Cue. */
export function updateModeForOption(
	option: RecordUpdateOption,
	settings: UpdateSettings,
	target: UpdateTargetRequest,
	configured: (settings: UpdateSettings, target: UpdateTargetRequest) => UpdateMode,
): UpdateMode | null {
	const cue = target.family.type === "cue";
	if (option === "merge")
		return cue
			? { target_type: "cue", mode: "add_new" }
			: { target_type: "existing_content", mode: "add_new" };
	if (option === "add_existing")
		return cue
			? { target_type: "cue", mode: "add_to_current_cue" }
			: { target_type: "existing_content", mode: "update_existing" };
	if (option === "add_cue" && cue) return null;
	return configured(settings, target);
}

const LEGACY_MODE_KEY = "light.store-mode";
const LEGACY_MERGE_ACTIVE_CUE_KEY = "light.store-merge-active-cue";

function legacyStorage(): Storage | null {
	try {
		const storage = globalThis.localStorage;
		return typeof storage?.getItem === "function" ? storage : null;
	} catch {
		return null;
	}
}

/**
 * The Record default the retired browser settings stand for. "Merge into active Cue" on becomes
 * Merge. The old Record mode was never applied, and its default cannot be told apart from an
 * explicit choice, so it is dropped. `null` means nothing is left to migrate.
 */
export function legacyRecordDefault(): RecordUpdateOption | "none" | null {
	const storage = legacyStorage();
	if (!storage) return null;
	const mode = storage.getItem(LEGACY_MODE_KEY);
	const merge = storage.getItem(LEGACY_MERGE_ACTIVE_CUE_KEY);
	if (mode === null && merge === null) return null;
	return merge === "true" ? "merge" : "none";
}

export function clearLegacyRecordDefaults() {
	const storage = legacyStorage();
	storage?.removeItem(LEGACY_MODE_KEY);
	storage?.removeItem(LEGACY_MERGE_ACTIVE_CUE_KEY);
}

/**
 * Moves the retired browser Record settings into the desk's stored default once. An explicit
 * desk default is never replaced. The browser keys are removed only after the desk accepted it.
 */
export async function migrateLegacyRecordDefaults(update: {
	loadSettings(): Promise<UpdateSettings | null>;
	saveSettings(settings: UpdateSettings): Promise<UpdateSettings | null>;
}) {
	const legacy = legacyRecordDefault();
	if (legacy === null) return false;
	if (legacy === "none") {
		clearLegacyRecordDefaults();
		return false;
	}
	const settings = await update.loadSettings();
	if (!settings) return false;
	if (settings.record_default !== "smart") {
		clearLegacyRecordDefaults();
		return false;
	}
	const saved = await update.saveSettings({
		...settings,
		record_default: legacy,
	});
	if (!saved) return false;
	clearLegacyRecordDefaults();
	return true;
}
