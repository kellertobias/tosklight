import { useRef, useState } from "react";
import { groups as fallbackGroups } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { StoredGroup, VersionedObject } from "../api/types";
import { useApp } from "../state/AppContext";
import { Button, Input } from "../components/common";
import { ButtonGrid, WindowHeader, WindowScrollArea } from "../components/window-kit";

export function GroupsWindow({ compact }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [contextGroup, setContextGroup] = useState<string | null>(null);
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
  const patched = new Set(
    server.patch?.fixtures.flatMap((fixture) => [
      fixture.fixture_id,
      ...fixture.logical_heads.map((head) => head.fixture_id),
    ]) ?? [],
  );
  const fixtureNames = new Map<string, string>();
  const capabilities = new Map<string, Set<string>>();
  for (const fixture of server.patch?.fixtures ?? []) {
    fixtureNames.set(
      fixture.fixture_id,
      `${fixture.definition.manufacturer} ${fixture.definition.model}`,
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
            patched={patched}
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
            select={() => {
              if (group && !state.storeArmed) return void server.selectGroup(group.id);
              if (group && state.storeArmed) { const mode = window.confirm("Merge the current selection into this group? Choose Cancel to overwrite it instead.") ? "merge" : "overwrite"; void server.storeGroup(group.id, group.body.name ?? `Group ${group.id}`, mode); dispatch({ type: "SET_STORE_ARMED", value: false }); return; }
              if (!state.storeArmed) return;
              void server.storeGroup(String(index + 1), `Group ${index + 1}`);
              dispatch({ type: "SET_STORE_ARMED", value: false });
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
              void server.selectGroup(contextual.id);
              setContextGroup(null);
            }}
          >
            Select live group
          </Button>
          <Button
            onClick={() => {
              void server.selectGroup(contextual.id, true);
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
                  void server.storeGroup(
                    contextual.id,
                    contextual.body.name ?? `Group ${contextual.id}`,
                  );
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
    </div>
  );
}

function GroupCard({
  group,
  index,
  patched,
  capabilities,
  selected,
  storeArmed,
  beginHold,
  cancelHold,
  openContext,
  select,
}: {
  group: VersionedObject<StoredGroup> | null;
  index: number;
  patched: Set<string>;
  capabilities: Map<string, Set<string>>;
  selected: boolean;
  storeArmed: boolean;
  beginHold: () => void;
  cancelHold: () => void;
  openContext: () => void;
  select: () => void;
}) {
  const missing =
    group?.body.fixtures.filter((fixture) => !patched.has(fixture)).length ?? 0;
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
        onPointerDown={beginHold}
        onPointerUp={cancelHold}
        onPointerCancel={cancelHold}
        onContextMenu={(event) => {
          event.preventDefault();
          openContext();
        }}
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
            {missing > 0 && <em>⚠ {missing} missing/unpatched</em>}
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
          </>
        ) : (
          <>
            <b>Empty</b>
            <small>{storeArmed ? "Tap to record empty group" : "Press Record to use this slot"}</small>
          </>
        )}
      </Button>;
}
