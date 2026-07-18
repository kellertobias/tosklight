import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { AttributeValue, Cue, CueList, VersionedObject, VisualizationSnapshot } from "../api/types";
import { cueVisualization, migrateStagePosition, renderStageThumbnail } from "./stage3dScene";
import { Button, FormField, FormLayout, HorizontalFaderField, ModalPortal, ModalTitleBar, NumberField, SearchBar, SelectField, SwitchField, TextField } from "../components/common";
import { ButtonGrid, WindowHeader, WindowScrollArea } from "../components/window-kit";
import { useApp } from "../state/AppContext";
import { cueUpdateTarget, requestUpdateTarget } from "../components/control/updateWorkflow";

function cueTriggerKind(cue: Cue | null | undefined): "go" | "follow" | "time" {
  if (cue?.trigger.type === "manual") return "go";
  if (cue?.trigger.type === "follow" && Number(cue.trigger.delay_millis ?? 0) === 0) return "follow";
  return "time";
}

function cueDraftIdentity(cue: Cue | null | undefined): string | null {
  if (!cue) return null;
  return cue.id ?? `number:${cue.number}`;
}

function formatCueSeconds(millis: number): string {
  return `${(millis / 1000).toFixed(3).replace(/\.?0+$/, "")} s`;
}

function legacyChaserXfadePercent(cueList: CueList, speedGroupsBpm: number[]): number {
  const groupIndex = cueList.speed_group ? cueList.speed_group.charCodeAt(0) - 65 : -1;
  const stepMillis = groupIndex >= 0
    ? Math.round(60_000 / Math.max(0.1, speedGroupsBpm[groupIndex] ?? 120) / Math.max(0.01, cueList.speed_multiplier ?? 1))
    : (cueList.chaser_step_millis ?? 1_000);
  return Math.min(100, Math.max(0, Math.round(((cueList.chaser_xfade_millis ?? 0) / Math.max(1, stepMillis)) * 100)));
}

function CuelistSettings({
  object,
  speedGroupsBpm,
  close,
  save,
}: {
  object: VersionedObject<CueList>;
  speedGroupsBpm: number[];
  close: () => void;
  save: (cueList: CueList, revision: number) => Promise<boolean>;
}) {
  const [draft, setDraft] = useState<CueList>({
    ...object.body,
    intensity_priority_mode: object.body.intensity_priority_mode ?? "htp",
    wrap_mode: object.body.wrap_mode ?? (object.body.looped ? "tracking" : "off"),
    restart_mode: object.body.restart_mode ?? "first_cue",
    force_cue_timing: object.body.force_cue_timing ?? false,
    disable_cue_timing: object.body.disable_cue_timing ?? false,
    chaser_xfade_millis: object.body.chaser_xfade_millis ?? 0,
    chaser_xfade_percent: object.body.chaser_xfade_percent ?? legacyChaserXfadePercent(object.body, speedGroupsBpm),
    speed_multiplier: object.body.speed_multiplier ?? 1,
  });
  const draftRef = useRef(draft);
  const priorityInputRef = useRef<HTMLInputElement>(null);
  const [renumberOpen, setRenumberOpen] = useState(false);
  const [startCue, setStartCue] = useState("");
  const [settingsError, setSettingsError] = useState("");
  const [renumberError, setRenumberError] = useState("");
  const [closeConfirm, setCloseConfirm] = useState(false);
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const initialDraft = useRef(JSON.stringify(draft));
  const replaceDraft = (next: CueList) => {
    draftRef.current = next;
    setDraft(next);
  };
  const update = <K extends keyof CueList>(key: K, value: CueList[K]) =>
    replaceDraft({ ...draftRef.current, [key]: value });
  const dirty = () => JSON.stringify(draftRef.current) !== initialDraft.current
    || String(priorityInputRef.current?.value ?? object.body.priority) !== String(object.body.priority);
  const requestClose = () => dirty() ? setCloseConfirm(true) : close();
  const submit = async () => {
    setSettingsError("");
    const priority = Number(priorityInputRef.current?.value ?? object.body.priority);
    if (!Number.isInteger(priority) || priority < -32_768 || priority > 32_767) {
      setSettingsError("Numeric priority must be a whole number from -32768 to 32767.");
      return;
    }
    const next = { ...draftRef.current, priority };
    if (!Number.isInteger(next.chaser_xfade_percent) || (next.chaser_xfade_percent ?? 0) < 0 || (next.chaser_xfade_percent ?? 0) > 100) {
      setSettingsError("Chaser X-fade must be a whole percentage from 0% to 100%.");
      return;
    }
    if (!Number.isFinite(next.speed_multiplier) || (next.speed_multiplier ?? 0) < 0.01 || (next.speed_multiplier ?? 0) > 100) {
      setSettingsError("Speed multiplier must be from 0.01× to 100×.");
      return;
    }
    next.chaser_xfade_millis = 0;
    if (await save(next, object.revision)) close();
    else setSettingsError("Unable to save Cuelist settings. Check the values or refresh after a revision conflict.");
  };
  const renumber = async () => {
    const start = startCue.trim() === "" ? 1 : Number(startCue);
    if (!Number.isSafeInteger(start) || start <= 0 || start + object.body.cues.length - 1 > Number.MAX_SAFE_INTEGER) {
      setRenumberError("Start Cue must be a positive whole number whose resulting Cue numbers are safe integers.");
      return;
    }
    const priority = Number(priorityInputRef.current?.value ?? object.body.priority);
    const next = {
      ...draftRef.current,
      priority,
      cues: object.body.cues.map((cue, index) => ({ ...cue, number: start + index })),
    };
    setRenumberError("");
    if (await save(next, object.revision)) {
      setRenumberOpen(false);
      close();
    } else setRenumberError("Renumbering was not applied. Refresh after a revision conflict and try again.");
  };
  useEffect(() => {
    if (!renumberOpen && !modeMenuOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      if (renumberOpen) {
        setRenumberOpen(false);
        setRenumberError("");
      } else setModeMenuOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape, true);
    return () => window.removeEventListener("keydown", closeOnEscape, true);
  }, [modeMenuOpen, renumberOpen]);
  const chooseMode = (mode: "sequence" | "chaser") => {
    const current = draftRef.current;
    replaceDraft({
      ...current,
      mode,
      speed_group: mode === "chaser" && current.speed_group == null ? "A" : current.speed_group,
    });
    setModeMenuOpen(false);
  };
  const modeControl = <div className="cuelist-mode-title-menu">
    <Button className="cuelist-mode-title-trigger" aria-haspopup="menu" aria-expanded={modeMenuOpen} onClick={() => setModeMenuOpen((open) => !open)}>
      <span>Mode</span><small>({draft.mode === "chaser" ? "Chaser" : "Sequence"})</small><i aria-hidden="true">▾</i>
    </Button>
    {modeMenuOpen && <div className="cuelist-mode-title-panel" role="menu" aria-label="Mode">
      <Button role="menuitemradio" aria-checked={draft.mode === "sequence"} onClick={() => chooseMode("sequence")}><span aria-hidden="true">{draft.mode === "sequence" ? "✓" : ""}</span>Sequence</Button>
      <Button role="menuitemradio" aria-checked={draft.mode === "chaser"} onClick={() => chooseMode("chaser")}><span aria-hidden="true">{draft.mode === "chaser" ? "✓" : ""}</span>Chaser</Button>
    </div>}
  </div>;
  const panel = (
      <section className="nested-modal cuelist-settings-modal" role="dialog" aria-modal="true" aria-label="Cuelist Settings">
        <ModalTitleBar
          title="Cuelist Settings"
          details={<><b>{draft.name}</b><small>{draft.cues.length} {draft.cues.length === 1 ? "Cue" : "Cues"}</small></>}
          actions={<>{modeControl}<Button disabled={!draft.cues.length} onClick={() => setRenumberOpen(true)}>Renumber Cues</Button><Button variant="primary" onClick={() => void submit()}>Save</Button></>}
          closeLabel="Close Cuelist Settings"
          onClose={requestClose}
        />
        <div className="cuelist-settings-columns">
          <section aria-labelledby="cuelist-priority-heading">
            <h3 id="cuelist-priority-heading">Priority</h3>
            <FormLayout labelPlacement="top">
              <NumberField
                label="Numeric priority"
                description="Resolves which Cuelist contribution wins before intensity HTP/LTP arbitration."
                min="-32768"
                max="32767"
                defaultValue={object.body.priority}
                ref={priorityInputRef}
              />
              <SelectField
                label="Intensity priority mode"
                description="HTP uses the highest intensity at the winning priority. LTP uses the newest intensity there; other attributes remain LTP."
                value={draft.intensity_priority_mode ?? "htp"}
                onChange={(value) => update("intensity_priority_mode", value)}
                options={[
                  { value: "htp", label: "HTP" },
                  { value: "ltp", label: "LTP" },
                ]}
              />
            </FormLayout>
          </section>
          <section aria-labelledby="cuelist-restart-heading">
            <h3 id="cuelist-restart-heading">Restart behavior</h3>
            <FormLayout labelPlacement="top">
              <SelectField
                label="Wrap Around"
                description="Off stops at the final Cue. Tracking returns to Cue 1 while retaining tracked values. Reset releases tracked state before Cue 1."
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
                description="First Cue starts at Cue 1 after Off. Continue Current Cue restores the Cue that was current before Off."
                value={draft.restart_mode ?? "first_cue"}
                onChange={(value) => update("restart_mode", value)}
                options={[
                  { value: "first_cue", label: "First Cue" },
                  { value: "continue_current_cue", label: "Continue Current Cue" },
                ]}
              />
            </FormLayout>
          </section>
          <section aria-labelledby="cuelist-timing-heading">
            <h3 id="cuelist-timing-heading">Timing</h3>
            <FormLayout labelPlacement="top">
              <SwitchField label="Force Cue Timing" description="Uses each Cue's Fade and Delay for every value, temporarily overriding stored per-value timing without deleting it." checked={draft.force_cue_timing ?? false} onChange={(event) => update("force_cue_timing", event.target.checked)} />
              <SwitchField label="Disable Cue Timing" description="Rehearsal bypass: makes Cue and per-value timing, TIME waits, and Chaser X-fade immediate without changing stored values. Chaser cadence continues; this overrides Force Cue Timing." checked={draft.disable_cue_timing ?? false} onChange={(event) => update("disable_cue_timing", event.target.checked)} />
              {draft.mode === "chaser" && <>
                <SelectField
                  label="Speed Group"
                  description="Supplies the live BPM used by this Chaser."
                  value={draft.speed_group ?? "legacy"}
                  onChange={(value) => update("speed_group", value === "legacy" ? null : value)}
                  options={[
                    ...(draft.speed_group == null ? [{ value: "legacy" as const, label: `Legacy fixed step (${(draft.chaser_step_millis ?? 1_000) / 1_000} s)`, disabled: true }] : []),
                    ...(["A", "B", "C", "D", "E"] as const).map((value) => ({ value, label: value })),
                  ]}
                />
                <NumberField label="Speed multiplier" description="Multiplies the selected Speed Group rate: 0.5× is half speed and 2× is double speed." unit="×" allowDecimal showStepButtons={false} min="0.01" max="100" step="0.01" value={draft.speed_multiplier ?? 1} onChange={(event) => update("speed_multiplier", Number(event.target.value))} />
                <HorizontalFaderField label="Chaser X-fade" description="Percentage of each effective Chaser step used to fade: 0% snaps and 100% fades for the full step." minimum={0} maximum={100} step={1} value={draft.chaser_xfade_percent ?? 0} display={`${draft.chaser_xfade_percent ?? 0}%`} onChange={(value) => update("chaser_xfade_percent", Math.round(value))} />
              </>}
            </FormLayout>
          </section>
        </div>
        {settingsError && <p className="ui-field-error" role="alert">{settingsError}</p>}
        {renumberOpen && (
          <div
            className="modal-backdrop"
            onPointerDown={(event) => {
              if (event.target !== event.currentTarget) return;
              setRenumberOpen(false);
              setRenumberError("");
            }}
          >
            <form
              className="modal-card"
              role="dialog"
              aria-modal="true"
              aria-label="Renumber Cues"
              onSubmit={(event) => {
                event.preventDefault();
                void renumber();
              }}
            >
              <Button
                className="modal-close"
                aria-label="Close Renumber Cues"
                onClick={() => {
                  setRenumberOpen(false);
                  setRenumberError("");
                }}
              >
                ×
              </Button>
              <h2>Renumber Cues</h2>
              <NumberField label="Start Cue" allowDecimal step="1" value={startCue} onChange={(event) => setStartCue(event.target.value)} />
              {renumberError && <p className="ui-field-error" role="alert">{renumberError}</p>}
              <div className="modal-actions">
                <Button
                  type="button"
                  onClick={() => {
                    setRenumberOpen(false);
                    setRenumberError("");
                  }}
                >
                  Cancel
                </Button>
                <Button type="submit">Renumber</Button>
              </div>
            </form>
          </div>
        )}
        {closeConfirm && (
          <div className="modal-backdrop">
            <section className="modal-card cuelist-settings-close-confirm" role="dialog" aria-label="Unsaved Cuelist Settings">
              <h2>Unsaved Cuelist Settings</h2>
              <p>Save the Cuelist changes, discard them, or stay in Cuelist Settings.</p>
              <div className="modal-actions three">
                <Button onClick={() => void submit()}>Save changes</Button>
                <Button className="danger" onClick={close}>Discard changes</Button>
                <Button onClick={() => setCloseConfirm(false)}>Stay</Button>
              </div>
            </section>
          </div>
        )}
      </section>
  );
  return <ModalPortal><div className="stacked-modal-layer cuelist-settings-backdrop" onPointerDown={(event) => event.target === event.currentTarget && requestClose()}>{panel}</div></ModalPortal>;
}

export function CuelistWindow({ builtIn = false, compact, cueListTab, showCueSidebar = true, cueListSource = "fixed", fixedCueListNumber }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [localTab, setLocalTab] = useState<"pool" | "cues">(cueListTab ?? "pool");
  const [localSelectedCuelist, setLocalSelectedCuelist] = useState<number>(1);
  const tab = builtIn ? state.cuelistBuiltInView : localTab;
  const pool = (server.playbacks?.pool ?? []).filter((definition) => definition.target.type === "cue_list").sort((left, right) => left.number - right.number);
  const firstAvailableCuelist = pool[0]?.number ?? 1;
  const paneSelectedCuelist = cueListSource === "follow-selection"
    ? (server.playbacks?.selected_playback ?? null)
    : (fixedCueListNumber ?? firstAvailableCuelist);
  const selectedCuelist = builtIn
    ? (state.cuelistBuiltInNumber ?? firstAvailableCuelist)
    : cueListTab === "cues"
      ? paneSelectedCuelist
      : localSelectedCuelist;
  const openCuelist = (number: number) => {
    if (builtIn) dispatch({ type: "OPEN_BUILTIN_CUELIST", number });
    else {
      setLocalSelectedCuelist(number);
      setLocalTab("cues");
    }
  };
  const openPool = () => (builtIn ? dispatch({ type: "SET_BUILTIN_CUELIST_VIEW", value: "pool" }) : setLocalTab("pool"));
  const [search, setSearch] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsCuelist, setSettingsCuelist] = useState<number | null>(null);
  const [cueListMessage, setCuelistMessage] = useState("");
  const holdTimer = useRef<number | null>(null);
  const held = useRef(false);
  const selectedPlaybackDefinition = server.playbacks?.pool.find((definition) => definition.number === selectedCuelist);
  const selectedDefinition = selectedPlaybackDefinition?.target.type === "cue_list" ? selectedPlaybackDefinition : undefined;
  const selectedCueListId = selectedDefinition?.target.type === "cue_list" ? selectedDefinition.target.cue_list_id : null;
  const legacyFirstCueObject = pool.length === 0 && selectedCuelist === 1 ? server.cueObjects?.[0] : undefined;
  const selectedCueObject = selectedCueListId ? server.cueObjects?.find((candidate) => candidate.id === selectedCueListId) : legacyFirstCueObject;
  const cueList = selectedCueObject?.body ?? (selectedCueListId
    ? server.playbacks?.cue_lists.find((candidate) => candidate.id === selectedCueListId)
    : pool.length === 0 && selectedCuelist === 1
      ? server.playbacks?.cue_lists[0]
      : undefined);
  const active = cueList && server.playbacks?.active.find((item) => item.cue_list_id === cueList.id);
  const cues = cueList?.cues ?? [];
  const [selectedCue, setSelectedCue] = useState(0);
  useEffect(() => {
    if (cueListTab !== "cues" || cueListSource !== "follow-selection" || active?.cue_index == null || !cues[active.cue_index]) return;
    setSelectedCue(active.cue_index);
  }, [active?.cue_index, cueListSource, cueListTab, cues]);
  const [cueDraft, setCueDraft] = useState<Cue | null>(null);
  const cueServerSnapshot = useRef<{ identity: string | null; serialized: string } | null>(null);
  const cuePropertiesRef = useRef<HTMLElement>(null);
  const cuePreviewRef = useRef<HTMLElement>(null);
  const cueSettingsGridRef = useRef<HTMLDivElement>(null);
  const cueTitleInputRef = useRef<HTMLInputElement>(null);
  const cueFadeInputRef = useRef<HTMLInputElement>(null);
  const cueDelayInputRef = useRef<HTMLInputElement>(null);
  const cueTriggerTimeInputRef = useRef<HTMLInputElement>(null);
  const cueTriggerPickerRef = useRef<HTMLDivElement>(null);
  const [cueFieldsFit, setCueFieldsFit] = useState(true);
  const [cueSettingsSetArmed, setCueSettingsSetArmed] = useState(false);
  const [cueTriggerModalOpen, setCueTriggerModalOpen] = useState(false);
  const [cueEditError, setCueEditError] = useState("");
  const cueSavePending = useRef("");
  const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
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
    const next = cues[selectedCue] ? { ...cues[selectedCue] } : null;
    const nextSnapshot = {
      identity: cueDraftIdentity(next),
      serialized: JSON.stringify(next),
    };
    const previousSnapshot = cueServerSnapshot.current;
    cueServerSnapshot.current = nextSnapshot;
    setCueDraft((current) => {
      const currentSerialized = JSON.stringify(current);
      const sameCue = nextSnapshot.identity != null
        && cueDraftIdentity(current) === nextSnapshot.identity
        && previousSnapshot?.identity === nextSnapshot.identity;
      const locallyEdited = sameCue && currentSerialized !== previousSnapshot.serialized;
      const serverCaughtUp = currentSerialized === nextSnapshot.serialized;
      // Event refreshes can return the last saved Cue while an operator is still typing. Keep
      // that local draft until the server catches up or the operator selects a different Cue.
      if (locallyEdited && !serverCaughtUp) return current;
      return next;
    });
  }, [selectedCue, cues]);
  useEffect(() => {
    setSelectedCue((current) => Math.min(current, Math.max(0, cues.length - 1)));
  }, [cueList?.id, cues.length]);
  useLayoutEffect(() => {
    const aside = cuePropertiesRef.current;
    const preview = cuePreviewRef.current;
    const fields = cueSettingsGridRef.current;
    if (!aside || !preview || !fields) return;
    const measure = () => {
      if (aside.clientHeight <= 0 || fields.scrollHeight <= 0) return;
      const style = getComputedStyle(aside);
      const available = aside.clientHeight - Number.parseFloat(style.paddingTop || "0") - Number.parseFloat(style.paddingBottom || "0");
      const gap = Number.parseFloat(style.rowGap || style.gap || "0");
      setCueFieldsFit(preview.offsetHeight + gap + fields.scrollHeight <= available + 1);
    };
    if (typeof ResizeObserver === "undefined") {
      measure();
      return;
    }
    const observer = new ResizeObserver(measure);
    observer.observe(aside);
    observer.observe(preview);
    observer.observe(fields);
    measure();
    return () => observer.disconnect();
  }, [compact, cueDraft?.id, cueDraft?.number, cueDraft?.trigger.type, cueListTab, tab]);
  useEffect(() => {
    if (cueFieldsFit) setCueSettingsSetArmed(false);
  }, [cueFieldsFit]);
  useEffect(() => {
    if (cueFieldsFit || tab !== "cues") return;
    const handleSet = (event: Event) => {
      if ((event as CustomEvent<string>).detail !== "set") return;
      setCueSettingsSetArmed((armed) => !armed);
    };
    window.addEventListener("light:desk-action", handleSet);
    return () => window.removeEventListener("light:desk-action", handleSet);
  }, [cueFieldsFit, tab]);
  const saveCue = async (nextCue = cueDraft) => {
    if (!nextCue || !selectedCueObject) return;
    const triggerDelay = nextCue.trigger.type === "manual"
      ? 0
      : typeof nextCue.trigger.delay_millis === "number"
        ? nextCue.trigger.delay_millis
        : Number.NaN;
    const timings: number[] = [
      nextCue.fade_millis,
      nextCue.delay_millis,
      triggerDelay,
    ];
    if (timings.some((value) => !Number.isSafeInteger(value) || value < 0)) {
      cueSavePending.current = "";
      setCueEditError("Cue edit was not saved. Fade, Delay, and Trigger time must be zero or greater.");
      return;
    }
    const saveKey = `${selectedCueObject.id}:${selectedCue}:${JSON.stringify(nextCue)}`;
    if (cueSavePending.current === saveKey) return;
    cueSavePending.current = saveKey;
    setCueEditError("");
    const saved = await server.saveCueList(
      {
        ...selectedCueObject.body,
        cues: selectedCueObject.body.cues.map((cue, index) => (index === selectedCue ? nextCue : cue)),
      },
      selectedCueObject.revision,
    );
    if (!saved) {
      cueSavePending.current = "";
      setCueEditError("Cue edit was not saved. Check the value or refresh after a revision conflict.");
    }
  };
  const settings = settingsOpen && settingsCueObject && (
    <CuelistSettings
      object={settingsCueObject}
      speedGroupsBpm={server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15]}
      close={() => setSettingsOpen(false)}
      save={server.saveCueList}
    />
  );
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
            search={<SearchBar value={search} onChange={setSearch} ariaLabel="Search Cuelists" placeholder="Number or name"/>}
            actions={[]}
          />
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
                  className={`pool-cell cuelist-card ${cueListDefinition ? "" : "empty"} ${runtime ? "running" : ""} ${selectedCuelist === number && cueListDefinition ? "selected" : ""} ${state.storeArmed ? "store-target" : ""} ${state.updateArmed ? "update-target" : ""} ${state.cueListSetTarget === number ? "set-target" : ""}`}
                  onPointerDown={() => {
                    if (!cueListDefinition || state.updateArmed) return;
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
                    if (state.updateArmed) {
                      const objectId = cueListDefinition?.target.type === "cue_list" ? cueListDefinition.target.cue_list_id : String(number);
                      requestUpdateTarget(cueUpdateTarget(objectId));
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
                      <small>{state.updateArmed ? "Touch to choose Update mode" : `Cuelist · ${runtime ? `${Math.round(runtime.master * 100)}%` : "Off"}`}</small>
                      <small>{usage.length ? `Playbacks on pages ${usage.join(", ")}` : "Not assigned to a playback"}</small>
                    </>
                  ) : (
                    <small>{state.updateArmed ? "Touch to check Update eligibility" : state.storeArmed ? "Tap to record Cuelist" : "Press Rec first"}</small>
                  )}
                </Button>
              );
            })}
          </ButtonGrid>
        </WindowScrollArea>
        {settings}
      </div>
    );
  const triggerKind = cueTriggerKind(cueDraft);
  const triggerMillis = Number(cueDraft?.trigger.delay_millis ?? 0);
  const showCueProperties = showCueSidebar && (!compact || cueListTab === "cues");
  const cueTableEmptyState = cueList
    ? {
        title: "This Cuelist has no Cues",
        description: "Record the first Cue to begin building this Cuelist.",
        icon: "▶",
      }
    : cueListTab === "cues" && cueListSource === "follow-selection"
      ? selectedCuelist == null
        ? {
            title: "No Cuelist selected",
            description: "Select a Cuelist playback and this pane will follow it.",
            icon: "◎",
          }
        : selectedPlaybackDefinition
          ? {
              title: "Selected playback is not a Cuelist",
              description: "Select a Cuelist playback for this pane to follow.",
              icon: "◎",
            }
          : {
              title: "Selected Cuelist is unavailable",
              description: "The selected playback no longer exists in the playback pool.",
              icon: "◎",
            }
      : {
          title: "Fixed Cuelist is unavailable",
          description: "Choose an available Cuelist in this pane's settings.",
          icon: "◎",
        };
  const openCueInput = (input: HTMLInputElement | null, buttonName: "Open keyboard" | "Open number pad") => {
    if (!cueSettingsSetArmed) return;
    setCueSettingsSetArmed(false);
    input?.closest(".ui-form-field")?.querySelector<HTMLButtonElement>(`button[aria-label="${buttonName}"]`)?.click();
  };
  const chooseCueTrigger = (value: "go" | "follow" | "time") => {
    if (!cueDraft) return;
    const trigger = value === "go" ? { type: "manual" } : value === "follow" ? { type: "follow", delay_millis: 0 } : { type: "wait", delay_millis: triggerMillis };
    const next = { ...cueDraft, trigger };
    setCueDraft(next);
    setCueTriggerModalOpen(false);
    void saveCue(next);
  };
  return (
    <div className="cuelist-window">
      {!compact && (
        <WindowHeader
          title={`Cuelist View · Cuelist ${selectedCuelist}${cueList?.name ? ` · ${cueList.name}` : ""}`}
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
      <div className={`sequence-layout ${showCueProperties ? "with-cue-properties" : ""}`}>
        <div className="cue-editor">
          <WindowScrollArea
            className="cue-table-wrap"
            emptyState={cues.length ? null : cueTableEmptyState}
          >
            {cues.length > 0 && (
              <table className="cue-table">
                <thead>
                  <tr>
                    <th>Preview</th>
                    <th>No.</th>
                    <th>Name</th>
                    <th>Trigger</th>
                    <th>Fade</th>
                  </tr>
                </thead>
                <tbody>
                  {cues.map((cue, index) => (
                    <tr
                      tabIndex={0}
                      aria-disabled={settingsOpen}
                      onClick={() => { if (!settingsOpen) setSelectedCue(index); }}
                      onKeyDown={(event) => {
                        if (settingsOpen) return;
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
                      <td>{cueTriggerKind(cue).toUpperCase()}</td>
                      <td>{(cue.fade_millis / 1000).toFixed(3).replace(/\.?0+$/, "")} s</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </WindowScrollArea>
        </div>
        {showCueProperties && (
          <aside ref={cuePropertiesRef} className={`sequence-actions cue-properties ${cueFieldsFit ? "" : "compact-cue-settings"}`.trim()}>
            {cueDraft && (
              <>
                <section ref={cuePreviewRef} className="cue-selected-preview">
                  {thumbnails[selectedCue] && <img className="cue-selected-thumbnail" src={thumbnails[selectedCue]} alt={`3D preview for Cue ${cueDraft.number}`} />}
                  <b className="cue-selected-label">Selected Cue · {cueDraft.number}</b>
                </section>
                <FormLayout labelPlacement="side" labelWidth={62} className="cue-settings-grid">
                  <div ref={cueSettingsGridRef} className="cue-settings-grid-measure">
                  <TextField
                    ref={cueTitleInputRef}
                    label="Title"
                    value={cueDraft.name}
                    onChange={(event) => setCueDraft({ ...cueDraft, name: event.target.value })}
                    onKeyboardCommit={(value) => {
                      const next = { ...cueDraft, name: value };
                      setCueDraft(next);
                      void saveCue(next);
                    }}
                    onBlur={(event) => void saveCue({ ...cueDraft, name: event.currentTarget.value })}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void saveCue({ ...cueDraft, name: event.currentTarget.value });
                    }}
                  />
                  <NumberField
                    ref={cueFadeInputRef}
                    label="Fade"
                    unit="s"
                    allowDecimal
                    min="0"
                    value={cueDraft.fade_millis / 1000}
                    onKeyboardCommit={(value) => {
                      const next = { ...cueDraft, fade_millis: Math.round(Number(value) * 1000) };
                      setCueDraft(next);
                      void saveCue(next);
                    }}
                    onChange={(event) =>
                      setCueDraft({
                        ...cueDraft,
                        fade_millis: Math.round(Number(event.target.value) * 1000),
                      })
                    }
                    onBlur={(event) =>
                      void saveCue({
                        ...cueDraft,
                        fade_millis: Math.round(Number(event.currentTarget.value) * 1000),
                      })
                    }
                    onKeyDown={(event) => {
                      if (event.key === "Enter")
                        void saveCue({
                          ...cueDraft,
                          fade_millis: Math.round(Number(event.currentTarget.value) * 1000),
                        });
                    }}
                  />
                  <NumberField
                    ref={cueDelayInputRef}
                    label="Delay"
                    unit="s"
                    allowDecimal
                    min="0"
                    value={cueDraft.delay_millis / 1000}
                    onKeyboardCommit={(value) => {
                      const next = { ...cueDraft, delay_millis: Math.round(Number(value) * 1000) };
                      setCueDraft(next);
                      void saveCue(next);
                    }}
                    onChange={(event) =>
                      setCueDraft({
                        ...cueDraft,
                        delay_millis: Math.round(Number(event.target.value) * 1000),
                      })
                    }
                    onBlur={(event) =>
                      void saveCue({
                        ...cueDraft,
                        delay_millis: Math.round(Number(event.currentTarget.value) * 1000),
                      })
                    }
                    onKeyDown={(event) => {
                      if (event.key === "Enter")
                        void saveCue({
                          ...cueDraft,
                          delay_millis: Math.round(Number(event.currentTarget.value) * 1000),
                        });
                    }}
                  />
                  <FormField label="Trigger">
                    <div className="cue-trigger-grid-control" ref={cueTriggerPickerRef}>
                      <SelectField
                        value={triggerKind}
                        onChange={(value) => {
                          const trigger = value === "go" ? { type: "manual" } : value === "follow" ? { type: "follow", delay_millis: 0 } : { type: "wait", delay_millis: triggerMillis };
                          const next = { ...cueDraft, trigger };
                          setCueDraft(next);
                          void saveCue(next);
                        }}
                        options={[
                          { value: "go", label: "GO" },
                          { value: "follow", label: "FOLLOW" },
                          { value: "time", label: "TIME" },
                        ]}
                      />
                      <Button size="compact" iconOnly aria-label="Open Trigger picker" onClick={() => cueTriggerPickerRef.current?.querySelector<HTMLButtonElement>(".ui-select-trigger")?.click()}><span className="ui-keyboard-icon" aria-hidden="true">⌨</span></Button>
                    </div>
                  </FormField>
                  {triggerKind === "time" && (
                      <NumberField
                      ref={cueTriggerTimeInputRef}
                      label="Trigger time"
                      unit="s"
                      allowDecimal
                      min="0"
                      value={triggerMillis / 1000}
                      onKeyboardCommit={(value) => {
                        const next = { ...cueDraft, trigger: { type: "wait", delay_millis: Math.round(Number(value) * 1000) } };
                        setCueDraft(next);
                        void saveCue(next);
                      }}
                      onChange={(event) =>
                        setCueDraft({
                          ...cueDraft,
                          trigger: {
                            type: "wait",
                            delay_millis: Math.round(Number(event.target.value) * 1000),
                          },
                        })
                      }
                      onBlur={(event) =>
                        void saveCue({
                          ...cueDraft,
                          trigger: {
                            type: "wait",
                            delay_millis: Math.round(Number(event.currentTarget.value) * 1000),
                          },
                        })
                      }
                      onKeyDown={(event) => {
                        if (event.key === "Enter")
                          void saveCue({
                            ...cueDraft,
                            trigger: {
                              type: "wait",
                              delay_millis: Math.round(Number(event.currentTarget.value) * 1000),
                            },
                          });
                      }}
                    />
                  )}
                  </div>
                </FormLayout>
                {!cueFieldsFit && <section className="cue-settings-compact-fallback" data-set-armed={cueSettingsSetArmed || undefined}>
                  <p>{cueSettingsSetArmed ? "SET is active. Press an attribute value to edit it." : "Press SET, then press an attribute value to edit it."}</p>
                  <div>
                    <Button aria-label="Set Cue Title" active={cueSettingsSetArmed} onClick={() => openCueInput(cueTitleInputRef.current, "Open keyboard")}><small>Title</small><b>{cueDraft.name || "Untitled"}</b></Button>
                    <Button aria-label="Set Cue Fade" active={cueSettingsSetArmed} onClick={() => openCueInput(cueFadeInputRef.current, "Open number pad")}><small>Fade</small><b>{formatCueSeconds(cueDraft.fade_millis)}</b></Button>
                    <Button aria-label="Set Cue Delay" active={cueSettingsSetArmed} onClick={() => openCueInput(cueDelayInputRef.current, "Open number pad")}><small>Delay</small><b>{formatCueSeconds(cueDraft.delay_millis)}</b></Button>
                    <Button aria-label="Set Cue Trigger" active={cueSettingsSetArmed} onClick={() => { if (!cueSettingsSetArmed) return; setCueSettingsSetArmed(false); setCueTriggerModalOpen(true); }}><small>Trigger</small><b>{triggerKind.toUpperCase()}</b></Button>
                    {triggerKind === "time" && <Button aria-label="Set Cue Trigger time" active={cueSettingsSetArmed} onClick={() => openCueInput(cueTriggerTimeInputRef.current, "Open number pad")}><small>Trigger time</small><b>{formatCueSeconds(triggerMillis)}</b></Button>}
                  </div>
                </section>}
                {cueEditError && <p className="ui-field-error" role="alert">{cueEditError}</p>}
              </>
            )}
          </aside>
        )}
      </div>
      {tab === "cues" && settings}
      {cueTriggerModalOpen && <ModalPortal><div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setCueTriggerModalOpen(false)}><section className="nested-modal cue-trigger-modal" role="dialog" aria-modal="true" aria-label="Cue Trigger"><ModalTitleBar title="Cue Trigger" closeLabel="Close Cue Trigger" onClose={() => setCueTriggerModalOpen(false)}/><div className="cue-trigger-options"><Button active={triggerKind === "go"} onClick={() => chooseCueTrigger("go")}>GO</Button><Button active={triggerKind === "follow"} onClick={() => chooseCueTrigger("follow")}>FOLLOW</Button><Button active={triggerKind === "time"} onClick={() => chooseCueTrigger("time")}>TIME</Button></div></section></div></ModalPortal>}
    </div>
  );
}
