import { Button } from "@tosklight/ui";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type {
	HighlightAction,
	HighlightFixtureSummary,
	HighlightState,
	PatchedFixture,
} from "../../api/types";
import { useSessionSnapshot } from "../../features/deskSnapshot/DeskSnapshotState";
import {
	useHighlightActions,
	useHighlightErrorMessage,
	useHighlightSnapshot,
} from "../../features/highlight/HighlightState";
import { usePatchView } from "../../features/patch/PatchContext";
import { usePatchFixturesById } from "../../features/patch/PatchState";
import type { SelectionGridConfiguration } from "../../features/programmingInteraction/contracts";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useOptionalApp } from "../../state/AppContext";
import { GridSettingsDialog } from "./selectionGrid/GridSettingsDialog";
import {
	createShiftAllPressGesture,
	type ShiftAllPressGesture,
} from "./selectionGrid/ShiftAllPressGesture";

function fixtureDetails(
	fixture: HighlightFixtureSummary | null,
	patch: readonly PatchedFixture[],
) {
	if (!fixture) return null;
	const patched = patch.find(
		(candidate) => candidate.fixture_id === fixture.fixture_id,
	);
	const number =
		fixture.number ?? fixture.fixture_number ?? patched?.fixture_number;
	const name =
		fixture.name?.trim() ||
		patched?.name?.trim() ||
		patched?.definition.name?.trim();
	const identity =
		number == null
			? `Fixture ${fixture.fixture_id.slice(0, 8)}`
			: `Fixture ${number}`;
	return name ? `${identity} · ${name}` : identity;
}

export function highlightStatusLabel(
	state: HighlightState | null,
	patch: readonly PatchedFixture[] = [],
) {
	if (!state) return "Unavailable";
	const total = state.remembered.length;
	if (state.mode !== "step")
		return total ? `ALL · ${total} selected` : "ALL · Empty selection";
	const index = state.active_index;
	const active =
		state.active_fixture ??
		(index == null ? null : (state.remembered[index] ?? null));
	const fixture = fixtureDetails(active, patch);
	const position =
		index == null ? `STEP · ${total}` : `STEP ${index + 1}/${total}`;
	return fixture ? `${position} · ${fixture}` : position;
}

function highlightAnnouncement(
	state: HighlightState | null,
	patch: readonly PatchedFixture[],
) {
	if (!state) return "Highlight state unavailable.";
	const status = `Highlight ${state.active ? "on" : "off"}. ${highlightStatusLabel(state, patch)}.`;
	const safety =
		state.capture_only || (state.active && !state.output_enabled)
			? " Live Highlight output suppressed."
			: "";
	return `${status}${safety}${state.message ? ` ${state.message}` : ""}`;
}

export function HighlightErrorAlert({
	message,
	onDismiss,
}: {
	message: string | null;
	onDismiss: () => void;
}) {
	if (!message) return null;
	return createPortal(
		<div className="highlight-error" data-highlight-error-alert role="alert">
			<span>{message}</span>
			<Button
				iconOnly
				aria-label="Dismiss Highlight error"
				onKeyDown={(event) => {
					if (event.key === "Enter" || event.key === " ")
						event.stopPropagation();
				}}
				onClick={onDismiss}
			>
				×
			</Button>
		</div>,
		document.body,
	);
}

/** Reads only the fixtures Highlight actually names, and only while they exist. */
function useHighlightPatchedFixtures(state: HighlightState | null) {
	const fixtureIds = (state?.remembered ?? [])
		.map((fixture) => fixture.fixture_id)
		.concat(state?.active_fixture ? [state.active_fixture.fixture_id] : []);
	const enabled = fixtureIds.length > 0;
	usePatchView(enabled);
	return usePatchFixturesById(fixtureIds, enabled);
}

export function HighlightControls() {
	const state = useHighlightSnapshot();
	const highlightError = useHighlightErrorMessage();
	const highlightActions = useHighlightActions();
	const selection = useProgrammingSelectionView();
	const selectionActions = useProgrammingSelectionActions();
	const app = useOptionalApp();
	const session = useSessionSnapshot();
	const [pending, setPending] = useState<HighlightAction | null>(null);
	const [gridPending, setGridPending] = useState(false);
	const [gridSettingsOpen, setGridSettingsOpen] = useState(false);
	const [gridError, setGridError] = useState<string | null>(null);
	const pendingRef = useRef(false);
	const shiftedAllPointer = useRef(false);
	const cycleGridRef = useRef<() => void>(() => {});
	const openGridSettingsRef = useRef<() => void>(() => {});
	const shiftAllGestureRef = useRef<ShiftAllPressGesture | null>(null);
	const patch = useHighlightPatchedFixtures(state);
	const ownedByOther = Boolean(
		state?.owner_user_id &&
			session?.user.id &&
			state.owner_user_id !== session.user.id,
	);
	const ownerLabel = state?.owner_user_name?.trim() || "another operator";
	const outputSuppressed = Boolean(
		state?.capture_only || (state?.active && !state.output_enabled),
	);

	const allowed = useCallback(
		(action: HighlightAction) => {
			if (pendingRef.current || !state) return false;
			if (ownedByOther && !state.capture_only) return false;
			if (action === "next") return state.can_next;
			if (action === "previous") return state.can_previous;
			return true;
		},
		[ownedByOther, state],
	);

	const invoke = useCallback(
		async (action: HighlightAction) => {
			if (!allowed(action) || !highlightActions) return;
			pendingRef.current = true;
			setPending(action);
			try {
				await highlightActions.highlightAction(action);
			} finally {
				pendingRef.current = false;
				setPending(null);
			}
		},
		[allowed, highlightActions],
	);

	const clearLatchedShift = useCallback(() => {
		if (app?.state.shiftArmed) {
			app.dispatch({ type: "SET_SHIFT_ARMED", value: false });
		}
	}, [app]);

	const invokeGrid = useCallback(
		async (action: "cycle" | "rows" | "columns") => {
			if (!selectionActions || gridPending) return;
			setGridError(null);
			setGridPending(true);
			try {
				const outcome =
					action === "cycle"
						? await selectionActions.cycleGridMethod()
						: await selectionActions.reorderFromGrid(action);
				if (!outcome)
					setGridError("The authoritative grid action could not be applied.");
			} finally {
				setGridPending(false);
			}
		},
		[gridPending, selectionActions],
	);

	cycleGridRef.current = () => void invokeGrid("cycle");
	openGridSettingsRef.current = () => {
		setGridError(null);
		setGridSettingsOpen(true);
	};
	useEffect(() => {
		const gesture = createShiftAllPressGesture({
			onCycleGridMethod: () => cycleGridRef.current(),
			onOpenGridSettings: () => openGridSettingsRef.current(),
		});
		shiftAllGestureRef.current = gesture;
		return () => {
			gesture.dispose();
			if (shiftAllGestureRef.current === gesture)
				shiftAllGestureRef.current = null;
		};
	}, []);

	useEffect(() => {
		const openGridSettings = () => openGridSettingsRef.current();
		window.addEventListener("light:selection-grid-settings", openGridSettings);
		return () =>
			window.removeEventListener(
				"light:selection-grid-settings",
				openGridSettings,
			);
	}, []);

	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (!event.altKey || event.ctrlKey || event.metaKey || event.repeat)
				return;
			const shifted = event.shiftKey || Boolean(app?.state.shiftArmed);
			const key = event.key.toLowerCase();
			if (shifted && key === "a") {
				event.preventDefault();
				event.stopPropagation();
				shiftAllGestureRef.current?.press();
				return;
			}
			if (
				shifted &&
				(event.key === "ArrowLeft" || event.key === "ArrowRight")
			) {
				event.preventDefault();
				event.stopPropagation();
				clearLatchedShift();
				void invokeGrid(event.key === "ArrowLeft" ? "columns" : "rows");
				return;
			}
			const action: HighlightAction | null =
				key === "h"
					? "toggle"
					: key === "a"
						? "all"
						: event.key === "ArrowLeft"
							? "previous"
							: event.key === "ArrowRight"
								? "next"
								: null;
			if (!action || !allowed(action)) return;
			event.preventDefault();
			event.stopPropagation();
			void invoke(action);
		};
		const onKeyUp = (event: KeyboardEvent) => {
			if (
				event.key.toLowerCase() !== "a" ||
				event.ctrlKey ||
				event.metaKey ||
				!shiftAllGestureRef.current?.isPressed()
			)
				return;
			event.preventDefault();
			event.stopPropagation();
			shiftAllGestureRef.current.release();
			clearLatchedShift();
		};
		const cancelGesture = () => shiftAllGestureRef.current?.cancel();
		window.addEventListener("keydown", onKeyDown);
		window.addEventListener("keyup", onKeyUp);
		window.addEventListener("blur", cancelGesture);
		return () => {
			window.removeEventListener("keydown", onKeyDown);
			window.removeEventListener("keyup", onKeyUp);
			window.removeEventListener("blur", cancelGesture);
		};
	}, [allowed, app?.state.shiftArmed, clearLatchedShift, invoke, invokeGrid]);

	const saveGridConfiguration = useCallback(
		async (configuration: SelectionGridConfiguration) => {
			if (!selectionActions || gridPending) return;
			setGridError(null);
			setGridPending(true);
			try {
				const outcome =
					await selectionActions.setGridConfiguration(configuration);
				if (outcome) setGridSettingsOpen(false);
				else
					setGridError(
						"Grid Settings could not be saved. Refresh and try again.",
					);
			} finally {
				setGridPending(false);
			}
		},
		[gridPending, selectionActions],
	);

	const invokeStep = (axis: "previous" | "next", shifted: boolean) => {
		if (shifted) {
			clearLatchedShift();
			void invokeGrid(axis === "previous" ? "columns" : "rows");
			return;
		}
		void invoke(axis);
	};

	const toggleLabel =
		ownedByOther && !state?.capture_only
			? `Highlight is controlled by ${ownerLabel}`
			: state?.active
				? "Turn Highlight off"
				: "Turn Highlight on";
	const details = state?.remembered
		.map((fixture) => fixtureDetails(fixture, patch))
		.filter(Boolean)
		.join(", ");
	const title = [
		highlightAnnouncement(state, patch),
		details ? `Live step source: ${details}.` : "",
		"Shortcuts: Alt+H HIGH, Alt+A ALL, Alt+Left/Right PREV/NEXT.",
	]
		.filter(Boolean)
		.join(" ");

	return (
		<section
			className={`highlight-controls ${state?.active ? "active" : ""} ${outputSuppressed ? "output-suppressed" : ""}`}
			aria-label="Highlight and selection stepping"
			aria-busy={pending !== null || gridPending}
			title={title}
		>
			<Button
				className={`highlight-toggle ${state?.active ? "highlight-armed" : "highlight-off"}`}
				data-keypad-key="HIGH"
				active={Boolean(state?.active)}
				aria-label={toggleLabel}
				aria-pressed={Boolean(state?.active)}
				aria-keyshortcuts="Alt+H"
				disabled={!allowed("toggle")}
				onClick={() => void invoke(state?.active ? "off" : "on")}
			>
				HIGH
			</Button>
			<Button
				className="highlight-previous"
				data-keypad-key="PREV"
				aria-label="Previous selection item"
				aria-keyshortcuts="Alt+ArrowLeft"
				disabled={
					app?.state.shiftArmed
						? !selectionActions || gridPending
						: !allowed("previous")
				}
				onClick={(event) =>
					invokeStep(
						"previous",
						event.shiftKey || Boolean(app?.state.shiftArmed),
					)
				}
			>
				PREV
			</Button>
			<Button
				className="highlight-next"
				data-keypad-key="NEXT"
				aria-label="Next selection item"
				aria-keyshortcuts="Alt+ArrowRight"
				disabled={
					app?.state.shiftArmed
						? !selectionActions || gridPending
						: !allowed("next")
				}
				onClick={(event) =>
					invokeStep("next", event.shiftKey || Boolean(app?.state.shiftArmed))
				}
			>
				NEXT
			</Button>
			<Button
				className="highlight-all"
				data-keypad-key="ALL"
				aria-label="Restore complete selection"
				aria-keyshortcuts="Alt+A"
				disabled={
					app?.state.shiftArmed
						? !selectionActions || gridPending
						: !allowed("all")
				}
				onPointerDown={(event) => {
					if (event.button !== 0 || !app?.state.shiftArmed) return;
					event.currentTarget.setPointerCapture?.(event.pointerId);
					shiftedAllPointer.current = true;
					shiftAllGestureRef.current?.press();
				}}
				onPointerUp={() => {
					if (!shiftedAllPointer.current) return;
					shiftAllGestureRef.current?.release();
					clearLatchedShift();
				}}
				onPointerCancel={() => {
					shiftedAllPointer.current = false;
					shiftAllGestureRef.current?.cancel();
					clearLatchedShift();
				}}
				onClick={(event) => {
					if (shiftedAllPointer.current) {
						shiftedAllPointer.current = false;
						event.preventDefault();
						return;
					}
					void invoke("all");
				}}
			>
				ALL
			</Button>
			<HighlightErrorAlert
				message={highlightError}
				onDismiss={() => highlightActions?.dismissHighlightError()}
			/>
			{gridSettingsOpen && selection ? (
				<GridSettingsDialog
					grid={selection.grid}
					busy={gridPending}
					error={gridError}
					onSave={(configuration) => void saveGridConfiguration(configuration)}
					onClose={() => {
						if (!gridPending) setGridSettingsOpen(false);
					}}
				/>
			) : null}
		</section>
	);
}
