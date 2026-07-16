import { useEffect, useMemo, useRef, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { AttributeValue, Cue, CueList, VersionedObject, VisualizationSnapshot } from "../api/types";
import { cueVisualization, migrateStagePosition, renderStageThumbnail } from "./stage3dScene";
import { Button, FormLayout, NumberField, SelectField, SwitchField, TextField, TextInput } from "../components/common";
import { ButtonGrid, WindowHeader, WindowScrollArea } from "../components/window-kit";
import { useApp } from "../state/AppContext";

function CuelistSettings({ object, close, save }: { object: VersionedObject<CueList>; close: () => void; save: (cueList: CueList, revision: number) => Promise<boolean> }) {
  const [draft, setDraft] = useState<CueList>({
    ...object.body,
    intensity_priority_mode: object.body.intensity_priority_mode ?? "htp",
    wrap_mode: object.body.wrap_mode ?? (object.body.looped ? "tracking" : "off"),
    restart_mode: object.body.restart_mode ?? "first_cue",
    force_cue_timing: object.body.force_cue_timing ?? false,
    disable_cue_timing: object.body.disable_cue_timing ?? false,
    chaser_xfade_millis: object.body.chaser_xfade_millis ?? 0,
    speed_multiplier: object.body.speed_multiplier ?? 1,
  });
  const [renumberOpen, setRenumberOpen] = useState(false);
  const [startCue, setStartCue] = useState("");
  const update = <K extends keyof CueList>(key: K, value: CueList[K]) => setDraft((current) => ({ ...current, [key]: value }));
  const submit = async () => {
    if (await save(draft, object.revision)) close();
  };
  const renumber = async () => {
    const start = startCue.trim() === "" ? 1 : Number(startCue);
    if (!Number.isSafeInteger(start) || start <= 0) return;
    const next = {
      ...draft,
      cues: draft.cues.map((cue, index) => ({ ...cue, number: start + index })),
    };
    if (await save(next, object.revision)) {
      setDraft(next);
      setRenumberOpen(false);
      close();
    }
  };
  return (
    <div className="modal-backdrop cuelist-settings-backdrop" onPointerDown={(event) => event.target === event.currentTarget && close()}>
      <section className="modal-card cuelist-settings-modal" role="dialog" aria-label="Cuelist Settings">
        <Button className="modal-close" onClick={close}>
          ×
        </Button>
        <h2>Cuelist Settings · {draft.name}</h2>
        <FormLayout labelPlacement="side">
          <SelectField
            label="Mode"
            value={draft.mode}
            onChange={(value) => update("mode", value)}
            options={[
              { value: "sequence", label: "Sequence" },
              { value: "chaser", label: "Chaser" },
            ]}
          />
          <NumberField label="Numeric priority" value={draft.priority} onChange={(event) => update("priority", Number(event.target.value))} />
          <SelectField
            label="Intensity priority mode"
            value={draft.intensity_priority_mode ?? "htp"}
            onChange={(value) => update("intensity_priority_mode", value)}
            options={[
              { value: "htp", label: "HTP" },
              { value: "ltp", label: "LTP" },
            ]}
          />
          <SelectField
            label="Wrap Around"
            value={draft.wrap_mode ?? (draft.looped ? "tracking" : "off")}
            onChange={(value) => update("wrap_mode", value)}
            options={[
              { value: "off", label: "Off" },
              { value: "tracking", label: "Tracking" },
              { value: "reset", label: "Reset" },
            ]}
          />
          <SelectField
            label="Restart mode"
            value={draft.restart_mode ?? "first_cue"}
            onChange={(value) => update("restart_mode", value)}
            options={[
              { value: "first_cue", label: "First Cue" },
              { value: "continue_current_cue", label: "Continue Current Cue" },
            ]}
          />
          <SwitchField label="Force Cue Timing" checked={draft.force_cue_timing ?? false} onChange={(event) => update("force_cue_timing", event.target.checked)} />
          <SwitchField label="Disable Cue Timing" checked={draft.disable_cue_timing ?? false} onChange={(event) => update("disable_cue_timing", event.target.checked)} />
          {draft.mode === "chaser" && (
            <>
              <SelectField
                label="Speed Group"
                value={draft.speed_group ?? "A"}
                onChange={(value) => update("speed_group", value)}
                options={(["A", "B", "C", "D", "E"] as const).map((value) => ({
                  value,
                  label: value,
                }))}
              />
              <SelectField
                label="Speed multiplier"
                value={String(draft.speed_multiplier ?? 1)}
                onChange={(value) => update("speed_multiplier", Number(value))}
                options={[0.25, 0.5, 1, 2, 4].map((value) => ({
                  value: String(value),
                  label: `${value}×`,
                }))}
              />
              <NumberField label="Chaser X-fade" unit="s" allowDecimal min="0" value={(draft.chaser_xfade_millis ?? 0) / 1000} onChange={(event) => update("chaser_xfade_millis", Number(event.target.value) * 1000)} />
            </>
          )}
        </FormLayout>
        <div className="modal-actions three">
          <Button onClick={close}>Cancel</Button>
          <Button disabled={!draft.cues.length} onClick={() => setRenumberOpen(true)}>
            Renumber Cues
          </Button>
          <Button onClick={() => void submit()}>Save</Button>
        </div>
        {renumberOpen && (
          <div className="modal-backdrop" onPointerDown={(event) => event.target === event.currentTarget && setRenumberOpen(false)}>
            <form
              className="modal-card"
              aria-label="Renumber Cues"
              onSubmit={(event) => {
                event.preventDefault();
                void renumber();
              }}
            >
              <h2>Renumber Cues</h2>
              <NumberField label="Start Cue" min="1" step="1" value={startCue} onChange={(event) => setStartCue(event.target.value)} />
              <div className="modal-actions">
                <Button type="button" onClick={() => setRenumberOpen(false)}>
                  Cancel
                </Button>
                <Button type="submit">Renumber</Button>
              </div>
            </form>
          </div>
        )}
      </section>
    </div>
  );
}

export function CuelistWindow({ builtIn = false, compact, cueListTab }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [localTab, setLocalTab] = useState<"pool" | "cues">(cueListTab ?? "pool");
  const [localSelectedCuelist, setLocalSelectedCuelist] = useState<number>(1);
  const tab = builtIn ? state.cuelistBuiltInView : localTab;
  const selectedCuelist = builtIn ? (state.cuelistBuiltInNumber ?? 1) : localSelectedCuelist;
  const openCuelist = (number: number) => (builtIn ? dispatch({ type: "OPEN_BUILTIN_CUELIST", number }) : (setLocalSelectedCuelist(number), setLocalTab("cues")));
  const openPool = () => (builtIn ? dispatch({ type: "SET_BUILTIN_CUELIST_VIEW", value: "pool" }) : setLocalTab("pool"));
  const [search, setSearch] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsCuelist, setSettingsCuelist] = useState<number | null>(null);
  const [cueListMessage, setCuelistMessage] = useState("");
  const holdTimer = useRef<number | null>(null);
  const held = useRef(false);
  const selectedDefinition = server.playbacks?.pool.find((cueList) => cueList.number === selectedCuelist && cueList.target.type === "cue_list");
  const selectedCueListId = selectedDefinition?.target.type === "cue_list" ? selectedDefinition.target.cue_list_id : null;
  const selectedCueObject = selectedCueListId ? server.cueObjects?.find((candidate) => candidate.id === selectedCueListId) : server.cueObjects?.[0];
  const cueList = selectedCueObject?.body ?? (selectedCueListId ? server.playbacks?.cue_lists.find((candidate) => candidate.id === selectedCueListId) : server.playbacks?.cue_lists[0]);
  const active = cueList && server.playbacks?.active.find((item) => item.cue_list_id === cueList.id);
  const cues = cueList?.cues ?? [];
  const [selectedCue, setSelectedCue] = useState(0);
  const [cueDraft, setCueDraft] = useState<Cue | null>(null);
  const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  const pool = (server.playbacks?.pool ?? []).filter((definition) => definition.target.type === "cue_list");
  const settingsDefinition = pool.find((definition) => definition.number === settingsCuelist);
  const settingsCueListId = settingsDefinition?.target.type === "cue_list" ? settingsDefinition.target.cue_list_id : null;
  const settingsCueObject = settingsCueListId ? server.cueObjects?.find((candidate) => candidate.id === settingsCueListId) : undefined;
  const poolByNumber = new Map(pool.map((playback) => [playback.number, playback]));
  const poolSlots = Array.from({ length: 1000 }, (_, index) => ({
    number: index + 1,
    playback: poolByNumber.get(index + 1) ?? null,
  }));
  const filteredPool = poolSlots.filter(({ number, playback }) => !search || playback?.name.toLowerCase().includes(search.toLowerCase()) || String(number).includes(search));
  const workflowMessage = state.cueListSetTarget != null ? `Cuelist ${state.cueListSetTarget} selected · touch a playback fader to assign it.` : state.cueListSetArmed ? "Select a Cuelist, then touch the playback fader where it should be assigned." : cueListMessage;
  const stageFixtures = useMemo(
    () =>
      (server.patch?.fixtures ?? []).flatMap((fixture, fixtureIndex) =>
        [
          {
            id: fixture.fixture_id,
            location: fixture.location,
            rotation: fixture.rotation,
          },
          ...(fixture.multipatch ?? []),
        ].map((instance, instanceIndex) => {
          const index = fixtureIndex * 16 + instanceIndex;
          const located =
            instance.location && (instance.location.x || instance.location.y || instance.location.z)
              ? {
                  x: instance.location.x / 1000,
                  y: instance.location.y / 1000,
                  z: instance.location.z / 1000,
                  rotationX: instance.rotation?.x ?? 0,
                  rotationY: instance.rotation?.y ?? 0,
                  rotationZ: instance.rotation?.z ?? 0,
                }
              : null;
          return {
            fixture,
            instanceId: instance.id,
            index,
            position: server.stageLayout?.body.positions3d?.[instance.id] ?? located ?? migrateStagePosition(instanceIndex ? undefined : server.stageLayout?.body.positions[fixture.fixture_id], index),
          };
        }),
      ),
    [server.patch, server.stageLayout],
  );
  useEffect(() => {
    if (!tauri || !cues.length || !stageFixtures.length) return;
    let cancelled = false;
    void server
      .readVisualization()
      .then((live) => {
        if (cancelled) return;
        let state: VisualizationSnapshot = { ...live, values: [] };
        const next: Record<number, string> = {};
        for (let index = 0; index < cues.length; index++) {
          const changes = [...(cues[index].changes ?? [])] as Array<{
            fixture_id: string;
            attribute: string;
            value: AttributeValue | null;
          }>;
          for (const groupChange of cues[index].group_changes ?? []) {
            const group = server.groups.find((candidate) => candidate.id === groupChange.group_id);
            for (const fixture_id of group?.body.fixtures ?? [])
              changes.push({
                fixture_id,
                attribute: groupChange.attribute,
                value: groupChange.value,
              });
          }
          state = cueVisualization(state, changes);
          next[index] = renderStageThumbnail(stageFixtures, state);
        }
        if (!cancelled) setThumbnails(next);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [tauri, cues, stageFixtures, server.groups, server.readVisualization]);
  useEffect(() => {
    setCueDraft(cues[selectedCue] ? { ...cues[selectedCue] } : null);
  }, [selectedCue, cues]);
  const saveCue = async () => {
    if (!cueDraft || !selectedCueObject) return;
    await server.saveCueList(
      {
        ...selectedCueObject.body,
        cues: selectedCueObject.body.cues.map((cue, index) => (index === selectedCue ? cueDraft : cue)),
      },
      selectedCueObject.revision,
    );
  };
  const deleteCue = async () => {
    if (!selectedCueObject || selectedCueObject.body.cues.length <= 1) return;
    const cues = selectedCueObject.body.cues.filter((_, index) => index !== selectedCue);
    if (await server.saveCueList({ ...selectedCueObject.body, cues }, selectedCueObject.revision)) {
      setSelectedCue(Math.min(selectedCue, cues.length - 1));
    }
  };
  const settings = settingsOpen && settingsCueObject && <CuelistSettings object={settingsCueObject} close={() => setSettingsOpen(false)} save={server.saveCueList} />;
  if (tab === "pool")
    return (
      <div className="cuelist-window cuelist-pool-window pool-window">
        {!compact && (
          <WindowHeader
            title="Cuelist Pool"
            info={{
              primary: `${pool.length} / 1000 Cuelists`,
              secondary: workflowMessage ? <span className="cuelist-workflow-status">{workflowMessage}</span> : undefined,
            }}
            actions={[]}
          />
        )}
        {!compact && (
          <label className="cuelist-search">
            <span>⌕ Search Cuelists</span>
            <TextInput clearable aria-label="Search Cuelists" placeholder="Number or name" value={search} onChange={(event) => setSearch(event.target.value)} />
          </label>
        )}
        {compact && workflowMessage && <div className="pool-message">{workflowMessage}</div>}
        <WindowScrollArea
          emptyState={
            filteredPool.length
              ? null
              : {
                  title: "No matching Cuelists",
                  description: `No Cuelist matches “${search}”.`,
                  icon: "⌕",
                }
          }
        >
          <ButtonGrid className="card-pool cuelist-pool-grid">
            {filteredPool.map(({ number, playback: cueListDefinition }) => {
              const runtime = cueListDefinition && server.playbacks?.active.find((item) => item.playback_number === cueListDefinition.number);
              const usage = cueListDefinition ? (server.playbacks?.pages ?? []).filter((page) => Object.values(page.slots).includes(cueListDefinition.number)).map((page) => page.number) : [];
              return (
                <Button
                  key={number}
                  className={`pool-cell cuelist-card ${cueListDefinition ? "" : "empty"} ${runtime ? "running" : ""} ${selectedCuelist === number && cueListDefinition ? "selected" : ""} ${state.storeArmed ? "store-target" : ""} ${state.cueListSetTarget === number ? "set-target" : ""}`}
                  onPointerDown={() => {
                    if (!cueListDefinition) return;
                    held.current = false;
                    holdTimer.current = window.setTimeout(() => {
                      held.current = true;
                      setSettingsCuelist(number);
                      setSettingsOpen(true);
                    }, 650);
                  }}
                  onPointerUp={() => {
                    if (holdTimer.current) window.clearTimeout(holdTimer.current);
                    holdTimer.current = null;
                  }}
                  onPointerCancel={() => {
                    if (holdTimer.current) window.clearTimeout(holdTimer.current);
                    holdTimer.current = null;
                  }}
                  onContextMenu={(event) => event.preventDefault()}
                  onClick={() => {
                    if (held.current) {
                      held.current = false;
                      return;
                    }
                    if (state.storeArmed) {
                      void server.executeCommandLine(`RECORD SET ${number}`).then(async (ok) => {
                        if (!ok) return;
                        server.setCommandLine("");
                        await server.refresh();
                        dispatch({ type: "SET_STORE_ARMED", value: false });
                      });
                      return;
                    }
                    if (state.cueListSetArmed) {
                      if (!cueListDefinition) {
                        setCuelistMessage(`Cuelist ${number} is empty · record it before assigning it to a playback.`);
                        return;
                      }
                      if (!builtIn) setLocalSelectedCuelist(number);
                      dispatch({
                        type: "SET_CUELIST_SET_TARGET",
                        value: number,
                      });
                      dispatch({ type: "SET_PRESET_SET_ARMED", value: false });
                      return;
                    }
                    if (!cueListDefinition) return;
                    setCuelistMessage("");
                    openCuelist(number);
                  }}
                >
                  <span className="number">{number}</span>
                  <b>{cueListDefinition?.name ?? "Empty"}</b>
                  {cueListDefinition ? (
                    <>
                      <small>Cuelist · {runtime ? `${Math.round(runtime.master * 100)}%` : "Off"}</small>
                      <small>{usage.length ? `Playbacks on pages ${usage.join(", ")}` : "Not assigned to a playback"}</small>
                    </>
                  ) : (
                    <small>{state.storeArmed ? "Tap to record Cuelist" : "Press Rec first"}</small>
                  )}
                </Button>
              );
            })}
          </ButtonGrid>
        </WindowScrollArea>
        {settings}
      </div>
    );
  const triggerKind = cueDraft?.trigger.type === "manual" ? "go" : cueDraft?.trigger.type === "follow" && Number(cueDraft.trigger.delay_millis ?? 0) === 0 ? "follow" : "time";
  const triggerMillis = Number(cueDraft?.trigger.delay_millis ?? 0);
  return (
    <div className="cuelist-window">
      {!compact && (
        <WindowHeader
          title={`Cues · ${cueList?.name ?? `Cuelist ${selectedCuelist}`}`}
          info={{
            primary: active ? "Running" : "Ready",
            secondary: `Revision ${selectedCueObject?.revision ?? 0}${cueList ? ` · ${cueList.mode} · priority ${cueList.priority}` : ""}`,
          }}
          actions={[
            [
              { id: "pool", label: "← Cuelist Pool", onClick: openPool },
              {
                id: "settings",
                label: "Cuelist Settings",
                onClick: () => {
                  setSettingsCuelist(selectedCuelist);
                  setSettingsOpen(true);
                },
              },
            ],
          ]}
        />
      )}
      <div className="sequence-layout">
        <div className="cue-editor">
          <WindowScrollArea
            className="cue-table-wrap"
            emptyState={
              cues.length
                ? null
                : {
                    title: "This Cuelist has no Cues",
                    description: "Record the first Cue to begin building this Cuelist.",
                    icon: "▶",
                  }
            }
          >
            {cues.length > 0 && (
              <table className="cue-table">
                <thead>
                  <tr>
                    <th>Preview image</th>
                    <th>Cue number</th>
                    <th>Cue name</th>
                    <th>Trigger</th>
                    <th>Fade time</th>
                  </tr>
                </thead>
                <tbody>
                  {cues.map((cue, index) => (
                    <tr
                      tabIndex={0}
                      onClick={() => setSelectedCue(index)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          setSelectedCue(index);
                        }
                      }}
                      key={cue.number}
                      className={`${active?.cue_index === index ? "current" : active?.cue_index === index - 1 ? "next" : ""} ${selectedCue === index ? "selected" : ""}`}
                    >
                      <td>{thumbnails[index] && <img src={thumbnails[index]} alt="" />}</td>
                      <td>
                        <b>{cue.number}</b>
                      </td>
                      <td>{cue.name || `Cue ${cue.number}`}</td>
                      <td>{cue.trigger.type === "manual" ? "GO" : cue.trigger.type.toUpperCase()}</td>
                      <td>{(cue.fade_millis / 1000).toFixed(3).replace(/\.?0+$/, "")} s</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </WindowScrollArea>
        </div>
        {!compact && (
          <aside className="sequence-actions cue-properties">
            {cueDraft && (
              <>
                <section>
                  {thumbnails[selectedCue] && <img className="cue-selected-thumbnail" src={thumbnails[selectedCue]} alt={`3D preview for Cue ${cueDraft.number}`} />}
                  <b>Selected Cue · {cueDraft.number}</b>
                </section>
                <FormLayout labelPlacement="top">
                  <TextField
                    label="Title"
                    value={cueDraft.name}
                    onChange={(event) => setCueDraft({ ...cueDraft, name: event.target.value })}
                    onBlur={() => void saveCue()}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void saveCue();
                    }}
                  />
                  <NumberField
                    label="Fade"
                    unit="s"
                    allowDecimal
                    min="0"
                    value={cueDraft.fade_millis / 1000}
                    onChange={(event) =>
                      setCueDraft({
                        ...cueDraft,
                        fade_millis: Number(event.target.value) * 1000,
                      })
                    }
                    onBlur={() => void saveCue()}
                  />
                  <NumberField
                    label="Delay"
                    unit="s"
                    allowDecimal
                    min="0"
                    value={cueDraft.delay_millis / 1000}
                    onChange={(event) =>
                      setCueDraft({
                        ...cueDraft,
                        delay_millis: Number(event.target.value) * 1000,
                      })
                    }
                    onBlur={() => void saveCue()}
                  />
                  <SelectField
                    label="Trigger"
                    value={triggerKind}
                    onChange={(value) => {
                      const trigger = value === "go" ? { type: "manual" } : value === "follow" ? { type: "follow", delay_millis: 0 } : { type: "wait", delay_millis: triggerMillis };
                      const next = { ...cueDraft, trigger };
                      setCueDraft(next);
                      if (selectedCueObject)
                        void server.saveCueList(
                          {
                            ...selectedCueObject.body,
                            cues: selectedCueObject.body.cues.map((cue, index) => (index === selectedCue ? next : cue)),
                          },
                          selectedCueObject.revision,
                        );
                    }}
                    options={[
                      { value: "go", label: "GO" },
                      { value: "follow", label: "FOLLOW" },
                      { value: "time", label: "TIME" },
                    ]}
                  />
                  {triggerKind === "time" && (
                    <NumberField
                      label="Trigger time"
                      unit="s"
                      allowDecimal
                      min="0"
                      value={triggerMillis / 1000}
                      onChange={(event) =>
                        setCueDraft({
                          ...cueDraft,
                          trigger: {
                            type: "wait",
                            delay_millis: Number(event.target.value) * 1000,
                          },
                        })
                      }
                      onBlur={() => void saveCue()}
                    />
                  )}
                </FormLayout>
                <Button className="large-danger danger" disabled={cues.length <= 1} onClick={() => void deleteCue()}>
                  Delete Cue
                </Button>
              </>
            )}
          </aside>
        )}
      </div>
      {settings}
    </div>
  );
}
