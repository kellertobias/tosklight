import type { ComponentType, ReactNode } from "react";
import type {
	ParameterFamily,
	SpecialParameterFamily,
} from "../../../control/parameterControls/model";
import { specialParameterFamilies } from "../../../control/parameterControls/model";
import { beamAttributesForFamily } from "../beamShapers";
import { ColorDialog } from "../color";
import { ControlDialog } from "../control";
import { FocusSpecialDialog } from "../focus/FocusSpecialDialog";
import { MediaPlayModeDialog } from "../media";
import { PositionDialog } from "../position";
import { PositionSpecialDialog } from "../semanticPosition/PositionSpecialDialog";
import { ShapersDialog } from "../shapers";
import { ColorSpecialDialog } from "../intention/color/ColorSpecialDialog";
import { SemanticSpecialDialogPlaceholder } from "./SemanticSpecialDialogPlaceholder";
import type { LegacySpecialDialogHost } from "./useLegacySpecialDialogHost";

/**
 * Special Dialog registry (TL-549/550/551 UI foundation).
 *
 * `SpecialDialogsModal` resolves the open family here instead of branching inline:
 *
 * - **Legacy entries** (programming contract 0, byte-for-byte as before the split) render a body
 *   inside the shared legacy card, from the legacy host state that stays mounted with the modal.
 * - **Semantic entries** (only when the backend reports the semantic contract) render their own
 *   `ModalFrame`. Position, Color and Focus start as placeholders; family agents replace the
 *   `Component` of their entry in {@link SEMANTIC_SPECIAL_DIALOGS} (or call
 *   {@link registerSemanticSpecialDialog}) and nothing else in the modal changes.
 *
 * Families without a semantic entry keep their legacy dialog even when semantic. Return Home
 * stays in the legacy Position dialog; it is not part of the semantic modal (owner decision).
 */

export interface SemanticSpecialDialogProps {
	family: SpecialParameterFamily;
	selectedFixtureIds: readonly string[];
	close(): void;
}

export interface LegacySpecialDialogEntry {
	mode: "legacy";
	family: SpecialParameterFamily;
	/** Extra class on the legacy `modal-card`. */
	cardClassName: string;
	render(host: LegacySpecialDialogHost): ReactNode;
}

export interface SemanticSpecialDialogEntry {
	mode: "semantic";
	family: SpecialParameterFamily;
	Component: ComponentType<SemanticSpecialDialogProps>;
}

export type SpecialDialogEntry = LegacySpecialDialogEntry | SemanticSpecialDialogEntry;

export const LEGACY_SPECIAL_DIALOGS: Readonly<
	Partial<Record<SpecialParameterFamily, LegacySpecialDialogEntry>>
> = {
	Position: {
		mode: "legacy",
		family: "Position",
		cardClassName: "position-special-dialog",
		render: (host) => <PositionDialog {...host.positionDialog} />,
	},
	Color: {
		mode: "legacy",
		family: "Color",
		cardClassName: "",
		render: (host) => (
			<ColorDialog {...host.colorDialog} shiftArmed={host.shiftArmed} />
		),
	},
	Shapers: {
		mode: "legacy",
		family: "Shapers",
		cardClassName: "shapers-special-dialog-card",
		render: (host) => (
			<ShapersDialog
				attributes={beamAttributesForFamily(host.available, "Shapers")}
				values={host.shaperValues}
				disabled={!host.valueWrites.canWrite}
				apply={host.apply}
			/>
		),
	},
	Media: {
		mode: "legacy",
		family: "Media",
		cardClassName: "",
		render: (host) => (
			<MediaPlayModeDialog
				choices={host.playModeChoices}
				value={host.playModeValue.value}
				mixed={host.playModeValue.mixed}
				disabled={!host.valueWrites.canWrite}
				apply={host.applyPlayMode}
			/>
		),
	},
	Control: {
		mode: "legacy",
		family: "Control",
		cardClassName: "",
		render: (host) => (
			<ControlDialog selectedFixtureIds={host.selectedFixtureIds} />
		),
	},
};

/** Extension point: family agents replace these placeholders with their production dialogs. */
export const SEMANTIC_SPECIAL_DIALOGS: Partial<
	Record<SpecialParameterFamily, SemanticSpecialDialogEntry>
> = {
	Position: {
		mode: "semantic",
		family: "Position",
		Component: PositionSpecialDialog,
	},
	Color: {
		mode: "semantic",
		family: "Color",
		Component: ColorSpecialDialog,
	},
	Focus: {
		mode: "semantic",
		family: "Focus",
		Component: FocusSpecialDialog,
	},
};

/** Installs a family's semantic Special Dialog (TL-549 Position, TL-550 Color, TL-551 Focus). */
export function registerSemanticSpecialDialog(
	family: SpecialParameterFamily,
	Component: ComponentType<SemanticSpecialDialogProps>,
) {
	SEMANTIC_SPECIAL_DIALOGS[family] = { mode: "semantic", family, Component };
}

/** The entry for `family`: semantic when the contract is active and one exists, else legacy. */
export function resolveSpecialDialog(
	family: string,
	semantic: boolean,
): SpecialDialogEntry | null {
	const key = family as SpecialParameterFamily;
	return (
		(semantic ? SEMANTIC_SPECIAL_DIALOGS[key] : undefined) ??
		LEGACY_SPECIAL_DIALOGS[key] ??
		null
	);
}

/** Whether the family tab offers a Special Dialog (Focus only under the semantic contract). */
export function hasSpecialDialog(family: ParameterFamily, semantic: boolean) {
	return semantic
		? resolveSpecialDialog(family, true) !== null
		: specialParameterFamilies.has(family as SpecialParameterFamily);
}
