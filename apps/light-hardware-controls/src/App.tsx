import { useState } from "react";
import { useHardwareController } from "./controller/useHardwareController";
import type { OscBridge } from "./transport/oscBridge";
import { GridSurface } from "./surfaces/GridSurface";
import { PlaybackSurface } from "./surfaces/PlaybackSurface";
import { ProgrammerSurface } from "./surfaces/ProgrammerSurface";
import { SettingsSurface } from "./surfaces/SettingsSurface";
import { NavigationRail } from "./surfaces/playback/NavigationRail";

type ControllerTab = "console" | "grid" | "settings";

export function App({ bridge }: { bridge?: OscBridge } = {}) {
  const controller = useHardwareController({ bridge });
  const [tab, setTab] = useState<ControllerTab>("console");
  const { feedback, settings, send } = controller;

  return (
    <main className={feedback.updateArmed ? "update-armed" : ""}>
      <header>
        <h1>ToskLight <span>Hardware Controls</span></h1>
        {feedback.updateArmed && (
          <strong className="hardware-update-state" role="status">
            UPDATE ARMED · touch an assigned playback
          </strong>
        )}
        <i className={feedback.connected ? "connected" : ""}>
          {feedback.connected
            ? `● Connected · page ${feedback.page}`
            : "○ Connecting…"}
        </i>
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
