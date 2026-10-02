import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
	colorDetailsRequests,
	useAcceptedColorReport,
	useColorDetailsRequest,
} from "../../../../../features/colorReport/useAcceptedColorReport";
import { useFamilyReadouts } from "../../../../../features/familyEncoders/useFamilyReadouts";
import { useApp } from "../../../../../state/AppContext";
import {
	encoderAreaStore,
	useEncoderAreaBudget,
} from "../../../../control/parameterControls/useEncoderArea";
import type { SemanticSpecialDialogProps } from "../../registry/specialDialogRegistry";
import { ColorDialogLayout, type ColorDialogPage } from "../ColorDialogLayout";
import type { ValueRange } from "../HorizontalRangeFader";
import { ColorAdoptionNoticePanel } from "./ColorAdoptionNoticePanel";
import { ColorApproximation } from "./ColorApproximation";
import { DirectColorSection } from "./DirectColorSection";
import { type ColorControlEdit, colorDialogControls } from "./ColorDialogControls";
import {
	type ColorDialogControl,
	type ColorDialogValues,
	colorComponentChange,
	isPendingEndpoint,
	requestedColorValues,
} from "./colorDialogModel";
import { useColorDialogLane } from "./useColorDialogLane";
import { useColorGestures } from "./useColorGestures";
import { useDirectColorControls } from "./useDirectColorControls";
import "./ColorSpecialDialog.css";

/** The inline owner name the encoder area knows this dialog by. */
export const COLOR_INLINE_OWNER = "color";

interface ColorDraft {
	/** The requested values the draft was made against; a newer request replaces the draft. */
	base: string;
	values: Partial<Record<ColorDialogControl, number>>;
	/** `null` removes that control's range (an ordinary edit collapses only that component). */
	ranges: Partial<Record<ColorDialogControl, ValueRange | null>>;
}

function applyDraft(
	requested: ColorDialogValues,
	draft: ColorDraft | null,
	base: string,
	fallbackHue: number,
	settling: boolean,
): ColorDialogValues {
	// A colourless request has no hue of its own: keep showing the last hue the operator saw.
	const hue =
		requested.saturation === 0 && !requested.ranges.hue ? fallbackHue : requested.hue;
	if (!draft || (draft.base !== base && !settling)) return { ...requested, hue };
	const ranges = { ...requested.ranges };
	for (const [control, range] of Object.entries(draft.ranges) as [ColorDialogControl, ValueRange | null][])
		if (range) ranges[control] = range;
		else delete ranges[control];
	return { ...requested, hue, ...draft.values, ranges };
}

/**
 * Optimistic presentation of the operator's own edits until the Programmer projection reports
 * the request back. The requested projection stays authoritative: any newer request replaces
 * the draft once every sent step is answered (`settling`); a reflection while steps are still
 * in flight is an older step's and keeps the draft.
 */
function useColorDraft(requested: ColorDialogValues, settling: boolean) {
	const base = JSON.stringify(requested);
	const [draft, setDraft] = useState<ColorDraft | null>(null);
	const lastHue = useRef(requested.hue);
	const shown = applyDraft(requested, draft, base, lastHue.current, settling);
	lastHue.current = shown.hue;
	const record = (edits: Parameters<ColorControlEdit>[0], pending: boolean) =>
		setDraft((current) => {
			const next: ColorDraft =
				current && (current.base === base || settling)
					? { base, values: { ...current.values }, ranges: { ...current.ranges } }
					: { base, values: {}, ranges: {} };
			for (const { control, value, range } of edits) {
				next.values[control] = value;
				if (!pending) next.ranges[control] = range ?? null;
			}
			return next;
		});
	return { shown, record };
}

/**
 * Production semantic Color Special Dialog (TL-550). Compact inside the measured lower encoder
 * area when it fits (≥ 680×210), the full shared `ModalFrame` otherwise or when expanded. Edits
 * run through one Color `FamilyGestureSession` on the lane the gesture started on; readouts are
 * the requested values; the approximation comes from the accepted-frame colour report.
 */
export function ColorSpecialDialog({ selectedFixtureIds, close }: SemanticSpecialDialogProps) {
	const { state } = useApp();
	const lane = useColorDialogLane(true, selectedFixtureIds);
	const readouts = useFamilyReadouts(lane.lane ?? "normal", lane.colorFixtureIds, {
		enabled: lane.lane !== null,
		consumerId: "color-dialog",
	});
	const gestures = useColorGestures(lane, readouts.reread);
	const budget = useEncoderAreaBudget();
	const [page, setPage] = useState<ColorDialogPage>("mix");
	const [expanded, setExpanded] = useState(false);
	const [focusFixtureId, setFocusFixtureId] = useState<string | null>(null);
	const details = useColorDetailsRequest();
	useEffect(() => {
		if (!details) return;
		setExpanded(true);
		setFocusFixtureId(details.fixtureId);
		colorDetailsRequests.clear();
	}, [details]);
	const cycle = useRef(budget.cycle);
	useEffect(() => {
		if (budget.cycle === cycle.current) return;
		cycle.current = budget.cycle;
		setPage((current) => (current === "mix" ? "white" : "mix"));
	}, [budget.cycle]);
	const modal = expanded || !budget.fits;
	useEffect(() => {
		if (modal) return;
		encoderAreaStore.claim(COLOR_INLINE_OWNER);
		return () => encoderAreaStore.release(COLOR_INLINE_OWNER);
	}, [modal]);
	const requested = requestedColorValues(
		lane.values,
		lane.colorFixtureIds,
		lane.descriptors,
	);
	const { shown, record } = useColorDraft(requested, gestures.settling);
	const media = lane.variant === "media";
	const edit: ColorControlEdit = (edits, gesture) => {
		const pending = edits.every((entry) => isPendingEndpoint(gesture.shifted, entry.range));
		record(edits, pending);
		if (pending) return;
		gestures.change(
			gesture,
			edits.map((entry) =>
				colorComponentChange(entry.control, entry.value, entry.range, lane.descriptors),
			),
		);
	};
	const controls = colorDialogControls({
		values: shown,
		descriptors: lane.descriptors,
		shiftArmed: state.shiftArmed,
		media,
		edit,
		gestures,
	});
	const reportIds = useMemo(
		() =>
			focusFixtureId && !lane.colorFixtureIds.includes(focusFixtureId)
				? [...lane.colorFixtureIds, focusFixtureId]
				: lane.colorFixtureIds,
		[focusFixtureId, lane.colorFixtureIds],
	);
	const report = useAcceptedColorReport(reportIds, {
		enabled: modal && !media,
		refreshKey: lane.values,
	});
	// TL-554: the Direct section is read only while the full modal shows it (inert reads).
	const direct = useDirectColorControls(lane, modal && !media && lane.lane !== null);
	const layout = (
		<ColorDialogLayout
			fits={budget.fits}
			page={page}
			expanded={expanded}
			onPage={setPage}
			onExpand={() => setExpanded(true)}
			onClose={close}
			compactPicker={controls.plane}
			whiteBlend={
				<>
					{controls.whiteBlend}
					<ColorAdoptionNoticePanel compact />
				</>
			}
			whiteBalance={controls.whiteBalance}
			expandedControls={controls.ring}
			approximation={
				<ColorApproximation
					requested={shown.preview}
					hueRange={shown.ranges.hue}
					saturation={shown.saturation}
					uvRequested={shown.uv}
					report={report}
					focusFixtureId={focusFixtureId}
				/>
			}
			native={
				<DirectColorSection
					controls={direct}
					report={report}
					fixtureIds={lane.colorFixtureIds}
				/>
			}
			mediaPreview={controls.mediaPreview}
			compactClassName="semantic-color-inline"
			modalClassName="semantic-color-modal"
		/>
	);
	return modal || !budget.element ? layout : createPortal(layout, budget.element);
}
