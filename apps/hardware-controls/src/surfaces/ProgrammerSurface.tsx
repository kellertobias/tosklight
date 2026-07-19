import {
  numericPadLayout,
  oscProgrammerActionForKey,
  softwareKeyLabel,
  type NumericPadSection,
} from "../../../shared/programmerKeypad";
import { ControlButton } from "../components/ControlButton";
import {
  darkLamp,
  type HighlightFeedback,
  type Lamp,
  type SendControl,
} from "../controller/types";
import { oscPaths } from "../oscPaths";
import {
  attachedHighlightKeys,
  attachedKeypadContentRowOffset,
  attachedProgrammerActionLayout,
} from "../programmerLayout";
import { TimeFader } from "./programmer/TimeFader";

interface ProgrammerSurfaceProps {
  updateArmed: boolean;
  lamps: Record<string, Lamp>;
  highlight: HighlightFeedback;
  send: SendControl;
}

export function ProgrammerSurface({
  updateArmed,
  lamps,
  highlight,
  send,
}: ProgrammerSurfaceProps) {
  const action = (name: string, down: boolean) => {
    send(oscPaths.programmer(name), [down]);
  };

  const renderKeypadSection = (section: NumericPadSection) =>
    numericPadLayout
      .filter((item) => item.section === section)
      .map(({ key, column, row, rowSpan = 1 }) => {
        const sectionColumn = section === "commands" ? column : column - 3;
        const displayRow = row + attachedKeypadContentRowOffset;
        const actionName = oscProgrammerActionForKey(key);
        return (
          <ControlButton
            key={key}
            keypadKey={key}
            className={`key-${actionName} ${key === "ENT" ? "key-enter" : ""}`}
            label={softwareKeyLabel(key)}
            style={{
              gridColumn: sectionColumn,
              gridRow: `${displayRow} / span ${rowSpan}`,
            }}
            onDown={() => action(actionName, true)}
            onUp={() => action(actionName, false)}
          />
        );
      });

  return (
    <aside className="programmer-panel">
      <div className="hardware-number-block">
        <div className="hardware-keypad-section hardware-keypad-command-section">
          <ControlButton
            keypadKey="RECORD"
            className="key-record"
            label={updateArmed ? "UPDATE" : "RECORD"}
            lamp={updateArmed
              ? { color: "#f4b942", state: "on" }
              : darkLamp}
            style={{
              gridColumn: attachedProgrammerActionLayout.record.column,
              gridRow: `${attachedProgrammerActionLayout.record.row} / span ${attachedProgrammerActionLayout.record.rowSpan}`,
            }}
            onDown={() => action("record", true)}
            onUp={() => action("record", false)}
          />
          <ControlButton
            keypadKey="PRELOAD GO"
            className="key-preload-go"
            label="PRELOAD GO"
            style={{
              gridColumn: attachedProgrammerActionLayout.preload.column,
              gridRow: `${attachedProgrammerActionLayout.preload.row} / span ${attachedProgrammerActionLayout.preload.rowSpan}`,
            }}
            onDown={() => action("preload", true)}
            onUp={() => action("preload", false)}
          />
          {renderKeypadSection("commands")}
        </div>
        <div className="hardware-keypad-section hardware-keypad-number-section">
          {attachedHighlightKeys.map((item) => (
            <ControlButton
              key={item.action}
              className={`highlight-key ${item.action === "toggle" ? "highlight-high" : `highlight-${item.action}`}`}
              label={item.label}
              lamp={highlightLamp(item.action, highlight, lamps.highlight)}
              keypadKey={item.label}
              showHoldFeedback={item.action !== "toggle"}
              style={{ gridColumn: item.column, gridRow: item.row }}
              onDown={() => send(oscPaths.highlight(item.action), [true])}
              onUp={() => send(oscPaths.highlight(item.action), [false])}
            />
          ))}
          {renderKeypadSection("numbers")}
        </div>
      </div>
      <div className="fade-times">
        <TimeFader
          label="Prog Fade"
          path="programmer/prog-fade"
          maximum={20}
          send={send}
        />
        <TimeFader
          label="Cue Fade"
          path="programmer/cue-fade"
          maximum={60}
          send={send}
        />
      </div>
    </aside>
  );
}

function highlightLamp(
  action: (typeof attachedHighlightKeys)[number]["action"],
  highlight: HighlightFeedback,
  toggleLamp: Lamp | undefined,
): Lamp {
  if (action === "toggle") return toggleLamp ?? darkLamp;
  if (action === "previous" && highlight.canPrevious) {
    return { color: "#68b9c7", state: "on" };
  }
  if (action === "next" && highlight.canNext) {
    return { color: "#68b9c7", state: "on" };
  }
  return darkLamp;
}
