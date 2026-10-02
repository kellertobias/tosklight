import { Button } from "@tosklight/ui";
import { EncoderSurfaces } from "./EncoderSurfaces";
import { ParameterFamilyTabs } from "./ParameterFamilyTabs";
import {
	encoderAreaStore,
	useEncoderArea,
	useEncoderAreaState,
} from "./useEncoderArea";
import type { ParameterController } from "./useParameterController";

/**
 * While the compact Color dialog occupies the encoder area (TL-550), the family tabs and the
 * Special Dialog key belong to it: tapping the active Color tab returns to the encoders on the
 * same encoder page (no paging), another family tab closes it and switches, and Special Dialog
 * cycles its pages.
 */
export function inlineDialogTabs(
	controller: ParameterController,
): ParameterController {
	return {
		...controller,
		selectEncoderGroup: (next, page) => {
			controller.dispatch({
				type: "SET_MODAL",
				modal: "specialDialogsOpen",
				value: false,
			});
			if (next !== controller.family) controller.selectEncoderGroup(next, page);
		},
	};
}

function InlineSpecialDialogButton() {
	return (
		<Button
			className="special-dialogs active"
			aria-label="Special Dialog"
			onClick={() => encoderAreaStore.requestCycle()}
		>
			<span className="special-dialog-label-full">
				<span>Special</span>
				<span>Dialog</span>
			</span>
			<span className="special-dialog-label-compact">Spcl</span>
		</Button>
	);
}

export function ParameterControlView({
	controller,
}: {
	controller: ParameterController;
}) {
	const area = useEncoderArea();
	const inline = useEncoderAreaState().inline;
	return (
		<div className="parameter-controls">
			<ParameterFamilyTabs
				controller={inline ? inlineDialogTabs(controller) : controller}
				specialDialog={inline ? <InlineSpecialDialogButton /> : undefined}
			/>
			<div
				ref={area.ref}
				className="parameter-surfaces"
				data-inline-dialog={inline ?? undefined}
				style={{
					gridTemplateColumns: `repeat(${controller.visibleEncoderCount}, minmax(0, 1fr))`,
				}}
			>
				{!inline && <EncoderSurfaces controller={controller} />}
			</div>
		</div>
	);
}
