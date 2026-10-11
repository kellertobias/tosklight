import { EncoderGroupTabs } from "@tosklight/ui/encoders";
import { compactFamilyLabels, parameterFamilyOrder } from "./model";
import { compactEncoderReadout, compactEncoderValueText } from "./CompactEncoderAssignments.readouts";
import type { ParameterController } from "./useParameterController";

/** A passive single row of the shared controller's current hardware assignments. */
export function CompactEncoderAssignments({ controller }: { controller: ParameterController }) {
	const context = controller.encoderContext ?? "programmer";
	const contextMessage = context === "dynamic-editor" ? "Dynamics editor — view its controls on the encoder screen"
		: context === "stage" ? "Stage navigation — view its controls on the encoder screen"
		: context === "timecode" ? "Timecode — view its controls on the encoder screen"
		: context === "patch" ? "Patch — view its controls on the encoder screen"
		: controller.dynamicsMode ? "Dynamics — select a parameter group to return to encoder assignments" : null;
	return (
		<section className="compact-encoder-assignments" aria-label="Hardware encoder assignments">
			<nav aria-label="Encoder groups and pages">
				<EncoderGroupTabs
					className="compact-encoder-groups"
					groups={parameterFamilyOrder.map((family) => ({
						id: family,
						label: family,
						compactLabel: compactFamilyLabels[family],
						pageCount: controller.familyEncoders?.pageCountFor(family) ?? controller.encoderGroups.find((group) => group.id === family.toLowerCase())?.pages.length ?? 1,
					}))}
					activeGroup={controller.family}
					page={controller.encoderPage}
					onChange={controller.selectEncoderGroup}
				/>
			</nav>
			{contextMessage ? <div className="compact-encoder-context" role="status">{contextMessage}</div> : <div className="compact-encoder-readouts" style={{ gridTemplateColumns: `repeat(${controller.visibleEncoderCount}, minmax(0, 1fr))` }} aria-label={`${controller.family} page ${controller.encoderPage} encoder values`}>
				{Array.from({ length: controller.visibleEncoderCount }, (_, index) => {
					const readout = compactEncoderReadout(controller, index);
					const description = `Encoder ${index + 1}: ${readout.label}, ${readout.text}${readout.detail ? `, ${readout.detail}` : ""}${readout.pushTurn ? `; Push-turn: ${readout.pushTurn.label}, ${readout.pushTurn.text}` : ""}`;
					return (
						<div key={index} className={`compact-encoder-readout ${readout.assigned ? "assigned" : "unassigned"}`} aria-label={description} title={description}>
							<header><small>{index + 1}</small><span>{readout.label}</span></header>
							<strong>{compactEncoderValueText(readout.text)}</strong>
							{readout.detail && <em>{readout.detail}</em>}
							{readout.pushTurn && <small className="compact-encoder-push-turn">Push-turn · {readout.pushTurn.label}: {compactEncoderValueText(readout.pushTurn.text)}</small>}
						</div>
					);
				})}
			</div>}
		</section>
	);
}
