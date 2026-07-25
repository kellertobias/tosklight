import { useEffect } from "react";
import type { PatchController } from "./controller";
import { usePatchController } from "./controller";
import { ensureSelectedSplitEdit } from "./editSession";
import { deleteFixture, requestFixtureDelete } from "./fixtureActions";

export function PatchEffects() {
	const controller = usePatchController();
	useSelectedSplitSetEffect(controller);
	usePatchDeleteKeys(controller);
	return null;
}

function useSelectedSplitSetEffect(controller: PatchController) {
	const selected = controller.data.selected;
	const editingSplit = controller.ui.editingSplit;
	const patchSetArmed = controller.appState.patchSetArmed;
	useEffect(() => {
		ensureSelectedSplitEdit(controller);
	}, [patchSetArmed, selected, editingSplit]);
}

function usePatchDeleteKeys(controller: PatchController) {
	const { ui } = controller;
	const { all } = controller.data;
	useEffect(() => {
		const handlePatchDeleteKeys = (event: KeyboardEvent) => {
			if (event.repeat || event.metaKey || event.ctrlKey || event.altKey)
				return;
			const target = event.target as HTMLElement | null;
			const tag = target?.tagName;
			const textTarget =
				target?.isContentEditable ||
				tag === "INPUT" ||
				tag === "TEXTAREA" ||
				tag === "SELECT";
			if (ui.deleteConfirm) {
				if (event.key === "Escape") {
					event.preventDefault();
					ui.setDeleteConfirm(null);
					return;
				}
				if (event.key === "Enter") {
					event.preventDefault();
					void deleteFixture(controller);
				}
				return;
			}
			if (
				textTarget ||
				ui.edit ||
				ui.multipatchEdit ||
				ui.browserOpen ||
				ui.placementOpen ||
				ui.layerModal ||
				ui.pending
			)
				return;
			if (event.key !== "Delete" && event.key !== "Backspace") return;
			const fixture = all.find(
				(item) => item.fixture_id === ui.selectedFixture,
			);
			if (!fixture) return;
			event.preventDefault();
			requestFixtureDelete(controller, fixture);
		};
		window.addEventListener("keydown", handlePatchDeleteKeys, true);
		return () =>
			window.removeEventListener("keydown", handlePatchDeleteKeys, true);
	}, [
		all,
		ui.browserOpen,
		ui.deleteConfirm,
		ui.edit,
		ui.layerModal,
		ui.multipatchEdit,
		ui.pending,
		ui.placementOpen,
		ui.selectedFixture,
	]);
}
