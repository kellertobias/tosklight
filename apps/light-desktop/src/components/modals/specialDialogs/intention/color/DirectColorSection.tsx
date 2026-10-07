import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import type { ColorIntentReport } from "../../../../../api/client/attributeConfiguration";
import { acceptedHeads } from "../../../../../features/colorReport/acceptedColorReport";
import { familySlotSpreads } from "../../../../control/parameterControls/familyEncoders/familyEncoderBinding";
import { nativeValueText } from "../../../../control/parameterControls/familyEncoders/nativeColorSlots";
import { ColorAdoptionNoticePanel } from "./ColorAdoptionNoticePanel";
import {
	directStatusRow,
	headName,
	referenceLabel,
	replayPreviewText,
} from "./directColorModel";
import type { DirectColorControls } from "./useDirectColorControls";

/**
 * TL-554 Direct (native) section of the full Color modal: the clearly identified reference
 * head and its picker, the native controls beyond encoder pages 3/4 (overflow), and the passive
 * per-head Direct status. Choosing a reference or reading rows sends nothing; only turning a
 * control edits, through the same binding as the encoders.
 */
export function DirectColorSection({
	controls,
	report,
	fixtureIds,
}: {
	controls: DirectColorControls;
	report: ColorIntentReport | null;
	fixtureIds: readonly string[];
}) {
	const pages = controls.pages;
	const reference = referenceLabel(pages);
	const rows = (acceptedHeads(report) ?? []).flatMap((head) => {
		const row = directStatusRow(head);
		return row ? [row] : [];
	});
	const previews = rows.length
		? []
		: fixtureIds.flatMap((id) => {
				const text = replayPreviewText(pages, id);
				// Named like every other row. A fixture without a verified native layout is no
				// reference candidate, but the colour report still names it.
				const candidate = pages?.candidates.find((entry) => entry.fixture_id === id);
				const reported = report?.heads.find((head) => head.fixture_id === id);
				const name = candidate
					? headName(candidate)
					: reported
						? headName({ ...reported, head_name: "" })
						: id;
				return text ? [{ id, name, text }] : [];
			});
	return (
		<div className="color-direct" data-testid="color-direct">
			<ColorAdoptionNoticePanel />
			{!pages?.reference ? (
				<p className="color-direct-quiet">
					{pages?.unavailable === "no_verified_head"
						? "No selected head has a verified native colour layout."
						: "Native colour pages are not available."}
				</p>
			) : (
				<>
					<p className="color-direct-reference" data-testid="color-direct-reference">
						Reference: {reference}
					</p>
					{pages.candidates.length > 1 && (
						<div className="color-direct-candidates" role="group" aria-label="Reference head">
							{pages.candidates.map((candidate) => (
								<Button
									key={`${candidate.fixture_id}:${candidate.head_id}`}
									aria-pressed={
										candidate.fixture_id === pages.reference?.fixture_id &&
										candidate.head_id === pages.reference?.head_id
									}
									onClick={() => controls.chooseReference(candidate)}
								>
									{headName(candidate)}
								</Button>
							))}
						</div>
					)}
					{controls.overflow.length > 0 && (
						<div className="color-direct-overflow" data-testid="color-direct-overflow">
							{controls.overflow.map((control, index) => (
								<div key={control.slot.id} className="color-direct-control">
									<TouchEncoder
										label={`Native ${index + 1} · ${control.slot.label}`}
										slot={index + 1}
										attributeLabel={control.slot.label}
										value={control.raw ?? control.slot.limits?.min ?? 0}
										display={
											control.raw === null ? "—" : nativeValueText(control.slot, control.raw)
										}
										disabled={control.slot.edit !== "scalar"}
										canRelease={false}
										onStep={(delta) => controls.step(control, delta)}
										onDragEnd={() => controls.finishGestures()}
										onSetRange={
											familySlotSpreads(control.slot)
												? (points) => controls.setRange(control, points)
												: undefined
										}
										minimum={control.slot.limits?.min ?? 0}
										maximum={control.slot.limits?.max ?? 0}
										inputScale={1}
										slowStep={control.slot.descriptor.step}
										fastStep={control.slot.descriptor.step * 10}
										onSet={(next) => controls.set(control, next)}
										onRelease={() => undefined}
									/>
									{control.choices.length > 0 && (
										<ul className="color-direct-choices" aria-label={`${control.slot.label} functions`}>
											{control.choices.map((choice) => (
												<li key={`${choice.label}:${choice.raw}`} data-current={choice.current}>
													<Button
														aria-pressed={choice.current}
														onClick={() => controls.set(control, choice.raw)}
													>
														{choice.label}
													</Button>
												</li>
											))}
										</ul>
									)}
								</div>
							))}
						</div>
					)}
				</>
			)}
			{(rows.length > 0 || previews.length > 0) && (
				<table className="color-direct-status" data-testid="color-direct-status">
					<tbody>
						{rows.map((row) => (
							<tr key={row.key}>
								<th scope="row">{row.name}</th>
								<td>{row.replay}</td>
								<td>{row.detail.join(" · ")}</td>
							</tr>
						))}
						{previews.map((preview) => (
							<tr key={preview.id}>
								<th scope="row">{preview.name}</th>
								<td>{preview.text}</td>
								<td />
							</tr>
						))}
					</tbody>
				</table>
			)}
		</div>
	);
}
