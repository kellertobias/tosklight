import { useRef, useState } from "react";
import type { PlaybackSurfaceLayout } from "../../api/types";
import {
  Button,
  FormLayout,
  ModalTitleBar,
  NumberField,
  SelectField,
  SwitchField,
} from "../common";
import { WindowScrollArea } from "../window-kit";

export function reorderPlaybackRows(
  rows: PlaybackSurfaceLayout["rows"],
  from: number,
  to: number,
) {
  if (from === to || from < 0 || to < 0 || from >= rows.length || to >= rows.length) return rows;
  const next = [...rows];
  const [row] = next.splice(from, 1);
  next.splice(to, 0, row);
  return next;
}

export function PlaybackLayoutModal({
  initialLayout,
  pageMode,
  pageModeLocked = false,
  onSave,
  onClose,
}: {
  initialLayout: PlaybackSurfaceLayout;
  pageMode: "follow_main" | "independent";
  pageModeLocked?: boolean;
  onSave: (layout: PlaybackSurfaceLayout, pageMode: "follow_main" | "independent") => void;
  onClose: () => void;
}) {
  const [layout, setLayout] = useState(() => structuredClone(initialLayout));
  const [draftPageMode, setDraftPageMode] = useState(pageMode);
  const dragRow = useRef<{ pointerId: number; from: number } | null>(null);
  const maxRows = 127;
  const maxFirst = 128 - layout.playbacks_per_row;
  const invalid =
    layout.playbacks_per_row < 1 ||
    layout.playbacks_per_row > 32 ||
    layout.rows.length === 0 ||
    layout.rows.length > maxRows ||
    layout.playbacks_per_row * layout.rows.length > 127 ||
    layout.rows.some(
      (row) =>
        row.first_playback_slot < 1 ||
        row.first_playback_slot > maxFirst ||
        row.button_count < 0 ||
        row.button_count > 3,
    );
  const updateRow = (index: number, changes: Partial<PlaybackSurfaceLayout["rows"][number]>) =>
    setLayout((current) => ({
      ...current,
      rows: current.rows.map((row, rowIndex) => (rowIndex === index ? { ...row, ...changes } : row)),
    }));
  const addRow = () =>
    setLayout((current) => {
      const previous = current.rows.at(-1);
      return {
        ...current,
        rows: [
          ...current.rows,
          {
            first_playback_slot: Math.min(
              128 - current.playbacks_per_row,
              (previous?.first_playback_slot ?? 1) + current.playbacks_per_row,
            ),
            has_fader: true,
            button_count: 3,
          },
        ],
      };
    });
  const moveRow = (from: number, to: number) => {
    if (from === to) return;
    setLayout((current) => ({
      ...current,
      rows: reorderPlaybackRows(current.rows, from, to),
    }));
  };

  return (
    <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
      <section className="nested-modal playback-layout-modal" role="dialog" aria-modal="true" aria-label="Configure Playbacks">
        <ModalTitleBar
          title="Configure Playbacks"
          actions={<>
            <Button disabled={layout.rows.length >= maxRows} onClick={addRow}>Add Row</Button>
            <Button className="playback-layout-save" variant="primary" disabled={invalid} onClick={() => onSave(layout, draftPageMode)}>Save</Button>
          </>}
          closeLabel="Close playback configuration"
          onClose={onClose}
        />
        <FormLayout columns={2} minColumnWidth={190}>
          <NumberField
            label="Playbacks per row"
            min="1"
            max="32"
            value={layout.playbacks_per_row}
            onChange={(event) => setLayout((current) => ({ ...current, playbacks_per_row: Number(event.target.value) }))}
          />
          <SelectField
            label="Page Mode"
            value={draftPageMode}
            disabled={pageModeLocked}
            onChange={(value) => setDraftPageMode(value as "follow_main" | "independent")}
            options={pageModeLocked
              ? [{ value: "follow_main", label: "Main Page" }]
              : [
                  { value: "follow_main", label: "Follow Main" },
                  { value: "independent", label: "Dedicated Page" },
                ]}
          />
        </FormLayout>
        {pageModeLocked && <small className="playback-page-mode-note">The default screen owns the main playback page.</small>}
        <WindowScrollArea className="playback-row-list">
          {layout.rows.map((row, index) => (
            <article
              className="playback-row-configuration"
              data-playback-row-index={index}
              key={index}
            >
              <Button
                className="playback-row-drag"
                aria-label={`Reorder playback row ${index + 1}`}
                title="Drag to reorder"
                onPointerDown={(event) => {
                  event.preventDefault();
                  dragRow.current = { pointerId: event.pointerId, from: index };
                  event.currentTarget.setPointerCapture?.(event.pointerId);
                }}
                onPointerMove={(event) => {
                  const active = dragRow.current;
                  if (!active || active.pointerId !== event.pointerId) return;
                  const target = document
                    .elementFromPoint(event.clientX, event.clientY)
                    ?.closest<HTMLElement>("[data-playback-row-index]");
                  const to = Number(target?.dataset.playbackRowIndex);
                  if (!Number.isInteger(to) || to === active.from) return;
                  moveRow(active.from, to);
                  dragRow.current = { ...active, from: to };
                }}
                onPointerUp={(event) => {
                  dragRow.current = null;
                  if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
                    event.currentTarget.releasePointerCapture(event.pointerId);
                  }
                }}
                onPointerCancel={(event) => {
                  dragRow.current = null;
                  if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
                    event.currentTarget.releasePointerCapture(event.pointerId);
                  }
                }}
              >
                <span aria-hidden="true">⠿</span>
              </Button>
              <NumberField
                label="First Playback Number"
                min="0"
                max={maxFirst}
                value={row.first_playback_slot}
                onChange={(event) => updateRow(index, { first_playback_slot: Number(event.target.value) })}
              />
              <SwitchField
                label="Fader"
                checked={row.has_fader}
                onChange={(event) => updateRow(index, { has_fader: event.target.checked })}
              />
              <NumberField
                label="Buttons"
                min="1"
                max="3"
                value={row.button_count}
                onChange={(event) => updateRow(index, { button_count: Number(event.target.value) })}
              />
              <Button
                className="playback-row-remove"
                variant="danger"
                iconOnly
                aria-label={`Remove row ${index + 1}`}
                title={`Remove row ${index + 1}`}
                disabled={layout.rows.length === 1}
                onClick={() => setLayout((current) => ({ ...current, rows: current.rows.filter((_, rowIndex) => rowIndex !== index) }))}
              >
                <svg aria-hidden="true" viewBox="0 0 24 24">
                  <path d="M4 7h16M9 7V4h6v3m-9 0 1 13h10l1-13M10 11v5m4-5v5" />
                </svg>
              </Button>
            </article>
          ))}
        </WindowScrollArea>
      </section>
    </div>
  );
}
