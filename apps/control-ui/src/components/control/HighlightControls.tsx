import { useCallback, useEffect, useRef, useState } from "react";
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
  if (!state.active) return total ? `${total} captured` : "No capture";
  if (state.mode === "selection") return total ? `All ${total}` : "No fixtures";
  const index = state.active_index;
  const active = state.active_fixture ?? (index == null ? null : state.remembered[index] ?? null);
  const fixture = fixtureDetails(active, patch);
  const position = index == null ? "Step" : `${index + 1}/${total}`;
  return fixture ? `${position} · ${fixture}` : position;
}

function highlightAnnouncement(state: HighlightState | null, patch: PatchedFixture[]) {
  if (!state) return "Highlight state unavailable.";
  const status = state.active ? `Highlight active, ${highlightStatusLabel(state, patch)}.` : `Highlight off, ${highlightStatusLabel(state, patch)}.`;
  const safety = state.capture_only || (state.active && !state.output_enabled) ? " Capture only; no live highlight output." : "";
  return `${status}${safety}${state.message ? ` ${state.message}` : ""}`;
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
  const captureOnly = Boolean(state?.capture_only || (state?.active && !state.output_enabled));

  const allowed = useCallback((action: HighlightAction) => {
    if (pendingRef.current || !state) return false;
    if (action === "capture") return server.selectedFixtures.length > 0;
    if (ownedByOther && !state.capture_only) return false;
    if (action === "next") return state.can_next;
    if (action === "previous") return state.can_previous;
    return true;
  }, [ownedByOther, server.selectedFixtures.length, state]);

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
      const action = key === "h"
        ? "toggle"
        : key === "c"
          ? "capture"
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
  const title = [highlightAnnouncement(state, patch), details ? `Remembered: ${details}.` : "", "Shortcuts: Alt+H toggle, Alt+C capture, Alt+Left/Right step."].filter(Boolean).join(" ");

  return (
    <section className={`highlight-controls ${state?.active ? "active" : ""} ${captureOnly ? "capture-only" : ""}`} aria-label="Highlight and step through" aria-busy={pending !== null} title={title}>
      <Button
        className="highlight-toggle"
        active={Boolean(state?.active)}
        aria-label={toggleLabel}
        aria-pressed={Boolean(state?.active)}
        aria-keyshortcuts="Alt+H"
        disabled={!allowed("toggle")}
        onClick={() => void invoke(state?.active ? "off" : "on")}
      >
        <b>HIGH</b>
        <small>{highlightStatusLabel(state, patch)}</small>
      </Button>
      <Button
        className="highlight-previous"
        aria-label="Previous highlighted fixture"
        aria-keyshortcuts="Alt+ArrowLeft"
        disabled={!allowed("previous")}
        onClick={() => void invoke("previous")}
      >
        PREV
      </Button>
      <Button
        className="highlight-next"
        aria-label="Next highlighted fixture"
        aria-keyshortcuts="Alt+ArrowRight"
        disabled={!allowed("next")}
        onClick={() => void invoke("next")}
      >
        NEXT
      </Button>
      <Button
        className="highlight-capture"
        aria-label="Capture current selection for Highlight"
        aria-keyshortcuts="Alt+C"
        disabled={!allowed("capture")}
        onClick={() => void invoke("capture")}
      >
        ALL
      </Button>
      {captureOnly && <span className="highlight-capture-only">CAPTURE ONLY</span>}
      {server.highlightError && (
        <div className="highlight-error" role="alert">
          <span>{server.highlightError}</span>
          <Button iconOnly aria-label="Dismiss Highlight error" onClick={server.dismissHighlightError}>×</Button>
        </div>
      )}
    </section>
  );
}
