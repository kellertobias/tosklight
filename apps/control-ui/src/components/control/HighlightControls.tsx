import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { HighlightAction, HighlightFixtureSummary, HighlightState, PatchedFixture } from "../../api/types";
import { useServer } from "../../api/ServerContext";
import { Button } from "../common";

function fixtureDetails(fixture: HighlightFixtureSummary | null, patch: PatchedFixture[]) {
  if (!fixture) return null;
  const patched = patch.find((candidate) => candidate.fixture_id === fixture.fixture_id);
  const number = fixture.number ?? fixture.fixture_number ?? patched?.fixture_number;
  const name = fixture.name?.trim() || patched?.name?.trim() || patched?.definition.name?.trim();
  const identity = number == null ? `Fixture ${fixture.fixture_id.slice(0, 8)}` : `Fixture ${number}`;
  return name ? `${identity} · ${name}` : identity;
}

export function highlightStatusLabel(state: HighlightState | null, patch: PatchedFixture[] = []) {
  if (!state) return "Unavailable";
  const total = state.remembered.length;
  if (state.mode !== "step") return total ? `ALL · ${total} selected` : "ALL · Empty selection";
  const index = state.active_index;
  const active = state.active_fixture ?? (index == null ? null : state.remembered[index] ?? null);
  const fixture = fixtureDetails(active, patch);
  const position = index == null ? `STEP · ${total}` : `STEP ${index + 1}/${total}`;
  return fixture ? `${position} · ${fixture}` : position;
}

function highlightAnnouncement(state: HighlightState | null, patch: PatchedFixture[]) {
  if (!state) return "Highlight state unavailable.";
  const status = `Highlight ${state.active ? "on" : "off"}. ${highlightStatusLabel(state, patch)}.`;
  const safety = state.capture_only || (state.active && !state.output_enabled)
    ? " Live Highlight output suppressed."
    : "";
  return `${status}${safety}${state.message ? ` ${state.message}` : ""}`;
}

export function HighlightErrorAlert({ message, onDismiss }: { message: string | null; onDismiss: () => void }) {
  if (!message) return null;
  return createPortal(<div className="highlight-error" data-highlight-error-alert role="alert">
    <span>{message}</span>
    <Button iconOnly aria-label="Dismiss Highlight error" onClick={onDismiss}>×</Button>
  </div>, document.body);
}

export function HighlightControls() {
  const server = useServer();
  const state = server.highlight;
  const [pending, setPending] = useState<HighlightAction | null>(null);
  const pendingRef = useRef(false);
  const patch = server.patch?.fixtures ?? [];
  const ownedByOther = Boolean(
    state?.owner_user_id
    && server.session?.user.id
    && state.owner_user_id !== server.session.user.id,
  );
  const ownerLabel = state?.owner_user_name?.trim() || "another operator";
  const outputSuppressed = Boolean(state?.capture_only || (state?.active && !state.output_enabled));

  const allowed = useCallback((action: HighlightAction) => {
    if (pendingRef.current || !state) return false;
    if (ownedByOther && !state.capture_only) return false;
    if (action === "next") return state.can_next;
    if (action === "previous") return state.can_previous;
    return true;
  }, [ownedByOther, state]);

  const invoke = useCallback(async (action: HighlightAction) => {
    if (!allowed(action)) return;
    pendingRef.current = true;
    setPending(action);
    try {
      await server.highlightAction(action);
    } finally {
      pendingRef.current = false;
      setPending(null);
    }
  }, [allowed, server]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.altKey || event.ctrlKey || event.metaKey || event.repeat) return;
      const key = event.key.toLowerCase();
      const action: HighlightAction | null = key === "h"
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
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [allowed, invoke]);

  const toggleLabel = ownedByOther && !state?.capture_only
    ? `Highlight is controlled by ${ownerLabel}`
    : state?.active
      ? "Turn Highlight off"
      : "Turn Highlight on";
  const details = state?.remembered.map((fixture) => fixtureDetails(fixture, patch)).filter(Boolean).join(", ");
  const title = [
    highlightAnnouncement(state, patch),
    details ? `Live step source: ${details}.` : "",
    "Shortcuts: Alt+H HIGH, Alt+A ALL, Alt+Left/Right PREV/NEXT.",
  ].filter(Boolean).join(" ");

  return (
    <section
      className={`highlight-controls ${state?.active ? "active" : ""} ${outputSuppressed ? "output-suppressed" : ""}`}
      aria-label="Highlight and selection stepping"
      aria-busy={pending !== null}
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
      >HIGH</Button>
      <Button
        className="highlight-previous"
        data-keypad-key="PREV"
        aria-label="Previous selection item"
        aria-keyshortcuts="Alt+ArrowLeft"
        disabled={!allowed("previous")}
        onClick={() => void invoke("previous")}
      >PREV</Button>
      <Button
        className="highlight-next"
        data-keypad-key="NEXT"
        aria-label="Next selection item"
        aria-keyshortcuts="Alt+ArrowRight"
        disabled={!allowed("next")}
        onClick={() => void invoke("next")}
      >NEXT</Button>
      <Button
        className="highlight-all"
        data-keypad-key="ALL"
        aria-label="Restore complete selection"
        aria-keyshortcuts="Alt+A"
        disabled={!allowed("all")}
        onClick={() => void invoke("all")}
      >ALL</Button>
      <HighlightErrorAlert message={server.highlightError} onDismiss={server.dismissHighlightError}/>
    </section>
  );
}
