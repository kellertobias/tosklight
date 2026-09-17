import { useState } from "react";
import { useHardwareController } from "./controller/useHardwareController";
import type { NativeHardwareBridge } from "./transport/nativeBridge";
import type { OscBridge } from "./transport/oscBridge";
import { GridSurface } from "./surfaces/GridSurface";
import { PlaybackSurface } from "./surfaces/PlaybackSurface";
import { ProgrammerSurface } from "./surfaces/ProgrammerSurface";
import { SettingsSurface } from "./surfaces/SettingsSurface";
import { LinkStatus, ModeSelector } from "./surfaces/ModeSelector";
import { NavigationRail } from "./surfaces/playback/NavigationRail";

type ControllerTab = "console" | "grid" | "settings";

interface AppProps {
  bridge?: OscBridge;
  nativeBridge?: NativeHardwareBridge;
}

export function App({ bridge, nativeBridge }: AppProps = {}) {
  const controller = useHardwareController({ bridge, nativeBridge });
  const [tab, setTab] = useState<ControllerTab>("console");
  const { feedback, settings, send } = controller;

  return (
    <main
      className={[
        feedback.updateArmed ? "update-armed" : "",
        controller.activeMode === "native" ? "native-mode" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <header>
        <h1>ToskLight <span>Hardware Controls</span></h1>
        {feedback.updateArmed && (
          <strong className="hardware-update-state" role="status">
            UPDATE ARMED · touch an assigned playback
          </strong>
        )}
        <ModeSelector
          activeMode={controller.activeMode}
          setMode={controller.setMode}
        />
        <LinkStatus
          activeMode={controller.activeMode}
          connected={feedback.connected}
          page={feedback.page}
          device={controller.device}
          lastInput={controller.lastInput}
          linkError={controller.linkError}
        />
      </header>
      <ControllerNavigation
        tab={tab}
        setTab={setTab}
        topRowVisible={settings.top}
        setTopRowVisible={controller.setTopRowVisible}
      />
      {tab === "console" ? (
        <section className="console-layout">
          <NavigationRail page={feedback.page} send={send} />
          <PlaybackSurface
            topRowVisible={settings.top}
            levels={feedback.levels}
            lamps={feedback.lamps}
            send={send}
          />
          <ProgrammerSurface
            updateArmed={feedback.updateArmed}
            lamps={feedback.lamps}
            highlight={feedback.highlight}
            send={send}
          />
        </section>
      ) : tab === "grid" ? (
        <GridSurface
          levels={feedback.levels}
          lamps={feedback.lamps}
          speedBpms={feedback.speedBpms}
          send={send}
        />
      ) : (
        <SettingsSurface
          connected={feedback.connected}
          activeMode={controller.activeMode}
          settings={settings}
          updateSettings={controller.updateSettings}
          connect={controller.connect}
        />
      )}
    </main>
  );
}

interface ControllerNavigationProps {
  tab: ControllerTab;
  setTab: (tab: ControllerTab) => void;
  topRowVisible: boolean;
  setTopRowVisible: (visible: boolean) => void;
}

function ControllerNavigation({
  tab,
  setTab,
  topRowVisible,
  setTopRowVisible,
}: ControllerNavigationProps) {
  return (
    <nav>
      <button
        className={tab === "console" ? "active" : ""}
        onClick={() => setTab("console")}
      >
        Playback Console
      </button>
      <button
        className={tab === "grid" ? "active" : ""}
        onClick={() => setTab("grid")}
      >
        Button Grid 41–90
      </button>
      <button
        className={tab === "settings" ? "active" : ""}
        onClick={() => setTab("settings")}
      >
        Settings
      </button>
      {tab === "console" && (
        <label>
          <input
            type="checkbox"
            checked={topRowVisible}
            onChange={(event) => setTopRowVisible(event.target.checked)}
          />{" "}
          Show 21–40
        </label>
      )}
    </nav>
  );
}
