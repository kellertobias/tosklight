import { Button } from "@tosklight/ui";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type {
	HighlightAction,
	HighlightFixtureSummary,
	HighlightState,
	PatchedFixture,
} from "../../api/types";
import {
	useHighlightActions,
	useHighlightErrorMessage,
	useHighlightSnapshot,
} from "../../features/highlight/HighlightState";
import { usePatchView } from "../../features/patch/PatchContext";
import { usePatchFixturesById } from "../../features/patch/PatchState";
import { useOptionalApp } from "../../state/AppContext";

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

function useHighlightInvocation(state: HighlightState | null) {
	const highlightError = useHighlightErrorMessage();
	const highlightActions = useHighlightActions();
	const [pending, setPending] = useState<HighlightAction | null>(null);
	const pendingRef = useRef(false);
	const allowed = useCallback(
		(action: HighlightAction) => {
			if (pendingRef.current || !state) return false;
			if (action === "next") return state.active && state.can_next;
			if (action === "previous") return state.active && state.can_previous;
			if (action === "all") return state.active;
			return true;
		},
		[state],
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
	return {
		allowed,
		dismissError: () => highlightActions?.dismissHighlightError(),
		error: highlightError,
		invoke,
		pending,
	};
}

function useHighlightKeyboardShortcuts({
	allowed,
	clearLatchedShift,
	invoke,
	shiftArmed,
}: {
	allowed: (action: HighlightAction) => boolean;
	clearLatchedShift: () => void;
	invoke: (action: HighlightAction) => Promise<void>;
	shiftArmed: boolean;
}) {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (!event.altKey || event.ctrlKey || event.metaKey || event.repeat)
				return;
			const shifted = event.shiftKey || shiftArmed;
			const key = event.key.toLowerCase();
			if (
				shifted &&
				(key === "a" || event.key === "ArrowLeft" || event.key === "ArrowRight")
			) {
				event.preventDefault();
				event.stopPropagation();
				clearLatchedShift();
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
		window.addEventListener("keydown", onKeyDown);
		return () => {
			window.removeEventListener("keydown", onKeyDown);
		};
	}, [allowed, clearLatchedShift, invoke, shiftArmed]);
}

function HighlightButtons({
	state,
	allowed,
	invoke,
	shiftArmed,
	clearLatchedShift,
}: {
	state: HighlightState | null;
	allowed: (action: HighlightAction) => boolean;
	invoke: (action: HighlightAction) => Promise<void>;
	shiftArmed: boolean;
	clearLatchedShift: () => void;
}) {
	const toggleLabel = state?.active
		? "Turn Highlight off"
		: "Turn Highlight on";
	return (
		<>
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
				disabled={!allowed("previous")}
				onClick={(event) => {
					if (event.shiftKey || shiftArmed) return clearLatchedShift();
					void invoke("previous");
				}}
			>
				PREV
			</Button>
			<Button
				className="highlight-next"
				data-keypad-key="NEXT"
				aria-label="Next selection item"
				aria-keyshortcuts="Alt+ArrowRight"
				disabled={!allowed("next")}
				onClick={(event) => {
					if (event.shiftKey || shiftArmed) return clearLatchedShift();
					void invoke("next");
				}}
			>
				NEXT
			</Button>
			<Button
				className="highlight-all"
				data-keypad-key="ALL"
				aria-label="Restore complete selection"
				aria-keyshortcuts="Alt+A"
				disabled={!allowed("all")}
				onClick={(event) => {
					if (event.shiftKey || shiftArmed) return clearLatchedShift();
					void invoke("all");
				}}
			>
				ALL
			</Button>
		</>
	);
}

export function HighlightControls() {
	const state = useHighlightSnapshot();
	const app = useOptionalApp();
	const patch = useHighlightPatchedFixtures(state);
	const outputSuppressed = Boolean(
		state?.capture_only || (state?.active && !state.output_enabled),
	);
	const highlight = useHighlightInvocation(state);
	const clearLatchedShift = useCallback(() => {
		if (app?.state.shiftArmed)
			app.dispatch({ type: "SET_SHIFT_ARMED", value: false });
	}, [app]);
	const shiftArmed = Boolean(app?.state.shiftArmed);
	useHighlightKeyboardShortcuts({
		allowed: highlight.allowed,
		clearLatchedShift,
		invoke: highlight.invoke,
		shiftArmed,
	});
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
			aria-busy={highlight.pending !== null}
			title={title}
		>
			<HighlightButtons
				state={state}
				allowed={highlight.allowed}
				invoke={highlight.invoke}
				shiftArmed={shiftArmed}
				clearLatchedShift={clearLatchedShift}
			/>
			<HighlightErrorAlert
				message={highlight.error}
				onDismiss={highlight.dismissError}
			/>
		</section>
	);
}
