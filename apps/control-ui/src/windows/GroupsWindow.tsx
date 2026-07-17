import { useEffect, useRef, useState } from "react";
import { groups as fallbackGroups } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { StoredGroup, VersionedObject } from "../api/types";
import { useApp } from "../state/AppContext";
import { Button, ColorPickerField, FormLayout, IconPickerField, Input, TextField } from "../components/common";
import { ButtonGrid, WindowHeader, WindowScrollArea } from "../components/window-kit";
import { RecordModeDialog, type RecordMode } from "../components/shared/RecordModeDialog";

export function GroupsWindow({ compact }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [contextGroup, setContextGroup] = useState<string | null>(null);
  const [recordGroup, setRecordGroup] = useState<string | null>(null);
  const [propertiesGroup, setPropertiesGroup] = useState<string | null>(null);
  const hold = useRef<number | null>(null);
  const fallback = (
    server.bootstrap
      ? []
      : fallbackGroups.map((group) => ({
          kind: "group",
          id: String(group.id),
          revision: 0,
          updated_at: "",
          body: {
            name: group.name,
            fixtures: Array.from({ length: group.fixtures }, (_, index) =>
              String(index),
            ),
            master: 1,
            playback_fader: group.id <= 8 ? group.id : null,
            programming: {},
            derived_from: null,
            frozen_from: null,
          },
        }))
  ) as typeof server.groups;
  const stored = server.bootstrap?.active_show ? server.groups : fallback;
  const cards = Array.from(
    { length: 40 },
    (_, index) =>
      stored.find((group) => group.id === String(index + 1)) ?? null,
  );
  const knownFixtureIds = new Set(
    server.patch?.fixtures.flatMap((fixture) => [
      fixture.fixture_id,
      ...fixture.logical_heads.map((head) => head.fixture_id),
    ]) ?? [],
  );
  const fixtureNames = new Map<string, string>();
  const capabilities = new Map<string, Set<string>>();
  for (const fixture of server.patch?.fixtures ?? []) {
    const fixtureLabel = fixture.fixture_number != null
      ? `Fixture ${fixture.fixture_number}`
      : (fixture.name || fixture.definition.name || fixture.fixture_id);
    fixtureNames.set(
      fixture.fixture_id,
      `${fixtureLabel} · ${fixture.definition.manufacturer} ${fixture.definition.model}`,
    );
    for (const head of fixture.definition.heads ?? []) {
      const owner = head.shared
        ? fixture.fixture_id
        : fixture.logical_heads.find(
            (candidate) => candidate.head_index === head.index,
          )?.fixture_id;
      if (owner) {
        fixtureNames.set(
          owner,
          head.shared
            ? fixtureNames.get(fixture.fixture_id)!
            : `${fixtureNames.get(fixture.fixture_id)} · head ${head.index}`,
        );
        capabilities.set(
          owner,
          new Set(head.parameters.map((parameter) => parameter.attribute)),
        );
      }
    }
  }
  const contextual = stored.find((group) => group.id === contextGroup);
  const recordTarget = stored.find((group) => group.id === recordGroup);
  const propertiesTarget = stored.find((group) => group.id === propertiesGroup);
  useEffect(() => {
    const openRequestedGroup = (event: Event) => {
      const id = (event as CustomEvent<string>).detail;
      if (stored.some((group) => group.id === id)) setPropertiesGroup(id);
    };
    window.addEventListener("light:group-configuration", openRequestedGroup);
    return () => window.removeEventListener("light:group-configuration", openRequestedGroup);
  }, [stored]);
  const cancelRecording = () => {
    setRecordGroup(null);
    dispatch({ type: "SET_STORE_ARMED", value: false });
  };
  const recordExistingGroup = async (mode: RecordMode) => {
    if (!recordTarget) return cancelRecording();
    await recordGroupCommand(recordTarget.id, mode);
    setRecordGroup(null);
    dispatch({ type: "SET_STORE_ARMED", value: false });
  };
  const runCommand = async (command: string, refresh = false) => {
    const ok = await server.executeCommandLine(command);
    if (ok && refresh) await server.refresh();
    return ok;
  };
  const recordGroupCommand = (id: string, mode: RecordMode = "overwrite") => {
    return runCommand(mode === "merge" ? `RECORD + GROUP ${id}` : `RECORD GROUP ${id}`, true);
  };
  const cancelHold = () => {
    if (hold.current) window.clearTimeout(hold.current);
    hold.current = null;
  };

  return (
    <div className="pool-window group-pool-window">
      {!compact && <WindowHeader title="Group Pool" info={{ primary: `${server.selectedFixtures.length} fixtures selected`, secondary: "Ordered selection" }} actions={[[...(state.groupsReturnToStage ? [{ id: "stage", label: "Back to Stage", onClick: () => dispatch({ type: "RETURN_TO_STAGE" }) }] : [])],[{ id: "presets", label: "Presets", onClick: () => dispatch({ type: "OPEN_BUILTIN", kind: "presets" }) }]]} />}
      <WindowScrollArea><ButtonGrid className="card-pool">
        {cards.map((group, index) => (
          <GroupCard
            key={index + 1}
            group={group}
            index={index}
            knownFixtureIds={knownFixtureIds}
            capabilities={capabilities}
            selected={server.selectedGroupId === group?.id}
            storeArmed={state.storeArmed}
            beginHold={() => {
              if (group)
                hold.current = window.setTimeout(
                  () => setContextGroup(group.id),
                  600,
                );
            }}
            cancelHold={cancelHold}
            openContext={() => group && setContextGroup(group.id)}
            dereference={() => group && runCommand(`DEGRP ${group.id}`)}
            select={() => {
              if (group && /^SET\b/i.test(server.commandLine.trim())) {
                setPropertiesGroup(group.id);
                server.resetCommandLine();
                return;
              }
              if (group && !state.storeArmed) return void server.selectionGesture({ type: "live_group", group_id: group.id });
              if (group && state.storeArmed) {
                if (!group.body.fixtures.length) {
                  void recordGroupCommand(group.id).finally(() =>
                    dispatch({ type: "SET_STORE_ARMED", value: false }),
                  );
                } else {
                  setRecordGroup(group.id);
                }
                return;
              }
              if (!state.storeArmed) return;
              void recordGroupCommand(String(index + 1)).finally(() =>
                dispatch({ type: "SET_STORE_ARMED", value: false }),
              );
            }}
          />
        ))}
      </ButtonGrid></WindowScrollArea>
      {contextual && (
        <div className="group-context-menu">
          <h3>{contextual.body.name ?? `Group ${contextual.id}`}</h3>
          <small className="group-order">
            Ordered members:{" "}
            {contextual.body.fixtures.length
              ? contextual.body.fixtures
                  .map(
                    (fixture, index) =>
                      `${index + 1}. ${fixtureNames.get(fixture) ?? fixture}`,
                  )
                  .join(" · ")
              : "empty"}
          </small>
          <label className="group-context-master">Master <strong>{Math.round((contextual.body.master ?? 1) * 100)}%</strong><Input aria-label={`${contextual.body.name ?? `Group ${contextual.id}`} master`} type="range" min="0" max="100" value={(contextual.body.master ?? 1) * 100} onChange={(event) => void server.setGroupMaster(contextual.id, Number(event.target.value) / 100)}/></label>
          <Button
            onClick={() => {
              runCommand(`GROUP ${contextual.id}`);
              setContextGroup(null);
            }}
          >
            Select live group
          </Button>
          <Button
            onClick={() => {
              runCommand(`GROUP GROUP ${contextual.id}`);
              setContextGroup(null);
            }}
          >
            Select frozen group
          </Button>
          {contextual.body.frozen_from && (
            <Button
              onClick={() => {
                void server.refreshFrozenGroup(contextual.id);
                setContextGroup(null);
              }}
            >
              Refresh frozen snapshot
            </Button>
          )}
          {contextual.body.derived_from ? (
            <Button
              onClick={() => {
                void server.detachDerivedGroup(contextual.id);
                setContextGroup(null);
              }}
            >
              Detach derived group
            </Button>
          ) : (
            <Button
              onClick={() => {
                const count = Object.keys(
                  contextual.body.programming ?? {},
                ).length;
                if (
                  !count ||
                  window.confirm(
                    `Replace membership and apply ${count} stored attributes to the new members?`,
                  )
                )
                  recordGroupCommand(contextual.id);
                setContextGroup(null);
              }}
            >
              Replace membership with selection
            </Button>
          )}
          <Button
            onClick={() => {
              void server.undoGroup(contextual.id);
              setContextGroup(null);
            }}
          >
            Undo membership/programming change
          </Button>
          <Button onClick={() => setContextGroup(null)}>Cancel</Button>
        </div>
      )}
      {recordTarget && (
        <RecordModeDialog
          target={recordTarget.body.name ?? `Group ${recordTarget.id}`}
          onChoose={recordExistingGroup}
          onCancel={cancelRecording}
        />
      )}
      {propertiesTarget && (
        <GroupPropertiesDialog
          key={`${propertiesTarget.id}:${propertiesTarget.revision}`}
          group={propertiesTarget}
          onClose={() => setPropertiesGroup(null)}
          onSave={async (update) => {
            if (await server.updateGroup(propertiesTarget.id, update)) setPropertiesGroup(null);
          }}
        />
      )}
    </div>
  );
}

function GroupCard({
  group,
  index,
  knownFixtureIds,
  capabilities,
  selected,
  storeArmed,
  beginHold,
  cancelHold,
  openContext,
  dereference,
  select,
}: {
  group: VersionedObject<StoredGroup> | null;
  index: number;
  knownFixtureIds: Set<string>;
  capabilities: Map<string, Set<string>>;
  selected: boolean;
  storeArmed: boolean;
  beginHold: () => void;
  cancelHold: () => void;
  openContext: () => void;
  dereference: () => void;
  select: () => void;
}) {
  const missing =
    group?.body.fixtures.filter((fixture) => !knownFixtureIds.has(fixture)).length ?? 0;
  const attributes = Object.keys(group?.body.programming ?? {});
  const unsupported =
    group?.body.fixtures.reduce(
      (count, fixture) =>
        count +
        attributes.filter(
          (attribute) =>
            capabilities.has(fixture) &&
            !capabilities.get(fixture)!.has(attribute),
        ).length,
      0,
    ) ?? 0;
  return <Button
        className={`group-card pool-cell ${group?.body.derived_from ? "derived" : ""} ${group?.body.frozen_from ? "frozen" : ""} ${selected ? "selected" : !group || !group.body.fixtures.length ? "empty" : ""} ${storeArmed && !group ? "store-target" : ""}`}
        style={group?.body.color ? { borderColor: group.body.color } : undefined}
        onPointerDown={beginHold}
        onPointerUp={cancelHold}
        onPointerCancel={cancelHold}
        onContextMenu={(event) => {
          event.preventDefault();
          openContext();
        }}
        onDoubleClick={dereference}
        onClick={select}
      >
        <span className="number">{index + 1}</span>
        {group ? (
          <>
            <b>{group.body.name ?? `Group ${index + 1}`}</b>
            <small>
              {group.body.fixtures.length
                ? `${group.body.fixtures.length} fixtures · ordered`
                : "⚠ Group is empty"}
            </small>
            {missing > 0 && <em>⚠ {missing} missing</em>}
            {attributes.length > 0 && (
              <em>{attributes.length} portable attributes</em>
            )}
            {unsupported > 0 && <em>⚠ {unsupported} unsupported values</em>}
            {group.body.derived_from && (
              <em>Derived · {group.body.derived_from.rule.type}</em>
            )}
            {group.body.frozen_from && (
              <em>Frozen · rev {group.body.frozen_from.source_revision}</em>
            )}
            {group.body.color && (
              <span
                className="group-color"
                aria-label={`Color ${group.body.color}`}
                style={{ background: group.body.color }}
              />
            )}
            {group.body.icon && <span className="group-icon" aria-label={`Icon ${group.body.icon}`}>{group.body.icon}</span>}
          </>
        ) : (
          <>
            <b>Empty</b>
            <small>{storeArmed ? "Tap to record empty group" : "Press Record to use this slot"}</small>
          </>
        )}
      </Button>;
}

function GroupPropertiesDialog({ group, onClose, onSave }: {
  group: VersionedObject<StoredGroup>;
  onClose: () => void;
  onSave: (update: Pick<StoredGroup, "name" | "color" | "icon">) => Promise<void>;
}) {
  const [name, setName] = useState(group.body.name ?? `Group ${group.id}`);
  const [color, setColor] = useState(group.body.color ?? "#718596");
  const [icon, setIcon] = useState(group.body.icon ?? "◇");
  const [saving, setSaving] = useState(false);
  const save = async () => {
    if (!name.trim() || saving) return;
    setSaving(true);
    await onSave({ name: name.trim(), color, icon });
    setSaving(false);
  };
  return <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal group-properties-modal" role="dialog" aria-modal="true" aria-label="Group properties">
      <Button className="modal-close" onClick={onClose}>×</Button>
      <h3>Group {group.id} properties</h3>
      <FormLayout labelPlacement="side">
        <TextField label="Group name" clearable autoFocus value={name} onChange={(event) => setName(event.target.value)}/>
        <ColorPickerField label="Color" value={color} onChange={setColor}/>
        <IconPickerField label="Icon" value={icon} onChange={setIcon}/>
      </FormLayout>
      <footer>
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={!name.trim() || saving} onClick={() => void save()}>{saving ? "Saving…" : "Save group"}</Button>
      </footer>
    </section>
  </div>;
}
