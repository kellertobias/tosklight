import { useSemanticFamilyEncoders } from "../../features/familyEncoders/FamilyEncodersProvider";
import { useApp } from "../../state/AppContext";
import { LegacySpecialDialogCard } from "./specialDialogs/registry/LegacySpecialDialogCard";
import { resolveSpecialDialog } from "./specialDialogs/registry/specialDialogRegistry";
import { useLegacySpecialDialogHost } from "./specialDialogs/registry/useLegacySpecialDialogHost";

export {
	type AuthoredFixtureControlChoice,
	type CompatibleFixtureControlAction,
	compatibleAuthoredControlActions,
	compatibleSpecialDialogActions,
} from "./specialDialogs/control";

/**
 * Resolves the open family's Special Dialog through the registry
 * (`specialDialogs/registry/specialDialogRegistry.tsx`). Under programming contract 0 every
 * family keeps its legacy dialog in the legacy card; under the semantic contract a family with a
 * semantic entry renders its own modal instead.
 */
export function SpecialDialogsModal() {
	const { state, dispatch } = useApp();
	const host = useLegacySpecialDialogHost(state);
	const semantic = useSemanticFamilyEncoders(
		host.selectedFixtureIds,
		state.specialDialogsOpen,
	);
	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });
	if (!state.specialDialogsOpen) return null;
	const family = state.specialDialogFamily;
	const entry = resolveSpecialDialog(family, semantic);
	if (entry?.mode === "semantic")
		return (
			<entry.Component
				family={entry.family}
				selectedFixtureIds={host.selectedFixtureIds}
				close={close}
			/>
		);
	return (
		<LegacySpecialDialogCard
			family={family}
			entry={entry}
			host={host}
			close={close}
		/>
	);
}
