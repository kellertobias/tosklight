/**
 * Shared server-domain contracts retained for test-module compatibility.
 *
 * Runtime state is owned by `ServerRuntime`; this module intentionally exposes
 * no context, provider, controller, or API facade.
 */
export {
	cueOnlyRestoration,
	deskLayoutScopeKey,
	type CommandChoiceOption,
	type PendingCommandChoice,
	type StagePosition3d,
	type StoredDeskLayout,
	type StoredStageLayout,
} from "../features/server/contracts";
