import { useState } from "react";
import type { DevelopmentView } from "../types";
import type { WindowProps } from "./windowTypes";
import {
  Button,
  CheckboxField,
  ColorPickerField,
  FormField,
  FormLayout,
  HorizontalFaderField,
  IconPickerField,
  MultiValueToggleField,
  NumberField,
  SelectField,
  SwitchField,
  TextField,
  type SelectOption,
} from "../components/common";
import { VerticalTouchFader } from "../components/control/VerticalTouchFader";
import { WindowFrame, WindowScrollArea } from "../components/window-kit";

export const DEVELOPMENT_VIEW_OPTIONS: SelectOption<DevelopmentView>[] = [
  { value: "forms", label: "Form elements" },
  { value: "faders", label: "Faders" },
  { value: "buttons", label: "Buttons" },
];

function FormElements() {
  const [name, setName] = useState("Front wash");
  const [level, setLevel] = useState(42.5);
  const [mode, setMode] = useState("merge");
  const [view, setView] = useState<"2d" | "3d">("2d");
  const [threeValues, setThreeValues] = useState("mid");
  const [fourValues, setFourValues] = useState("north");
  const [fiveValues, setFiveValues] = useState("3");
  const [sixValues, setSixValues] = useState("a");
  const [enabled, setEnabled] = useState(true);
  const [checked, setChecked] = useState(true);
  const [icon, setIcon] = useState("◇");
  const [color, setColor] = useState("#1bd6ec");
  const [brightness, setBrightness] = useState(1);
  return <div className="development-catalog">
    <section><h2>Side labels</h2><FormLayout labelPlacement="side">
      <TextField label="Text" clearable value={name} onChange={(event) => setName(event.target.value)}/>
      <NumberField label="Number" allowDecimal step={0.5} value={level} onChange={(event) => setLevel(Number(event.target.value))}/>
      <SelectField label="Dropdown" value={mode} onChange={setMode} options={[{ value: "merge", label: "Merge" }, { value: "overwrite", label: "Overwrite" }]}/>
      <MultiValueToggleField label="2 values" value={view} onChange={setView} options={[{ value: "2d", label: "2D" }, { value: "3d", label: "3D" }]}/>
      <MultiValueToggleField label="3 values" value={threeValues} onChange={setThreeValues} options={[{ value: "low", label: "Low" }, { value: "mid", label: "Mid" }, { value: "high", label: "High" }]}/>
      <MultiValueToggleField label="4 values" value={fourValues} onChange={setFourValues} options={[{ value: "north", label: "N" }, { value: "east", label: "E" }, { value: "south", label: "S" }, { value: "west", label: "W" }]}/>
      <MultiValueToggleField label="5 values" value={fiveValues} onChange={setFiveValues} options={["1", "2", "3", "4", "5"].map((value) => ({ value, label: value }))}/>
      <MultiValueToggleField label="6 values" value={sixValues} onChange={setSixValues} options={["a", "b", "c", "d", "e", "f"].map((value) => ({ value, label: value.toUpperCase() }))}/>
      <SwitchField label="Toggle" checked={enabled} onChange={(event) => setEnabled(event.target.checked)}/>
      <CheckboxField label="Checkbox" checked={checked} onChange={(event) => setChecked(event.target.checked)}/>
      <IconPickerField label="Icon" value={icon} onChange={setIcon}/>
      <ColorPickerField label="Color" value={color} onChange={setColor}/>
      <HorizontalFaderField label="Horizontal fader" value={brightness} minimum={0} maximum={2} step={0.05} display={`${Math.round(brightness * 100)}%`} onChange={setBrightness}/>
      <FormField label="Arbitrary content"><Button variant="primary">Form action</Button></FormField>
    </FormLayout></section>
    <section><h2>Top labels and columns</h2><FormLayout labelPlacement="top" columns={3} minColumnWidth={170}><TextField label="First column" value="A" readOnly/><TextField label="Second column" value="B" readOnly/><TextField label="Third column" value="C" readOnly/></FormLayout></section>
  </div>;
}

function FaderElements() {
  const [levels, setLevels] = useState([25, 50, 75, 100]);
  const update = (index: number, value: number) => setLevels((current) => current.map((item, itemIndex) => itemIndex === index ? value : item));
  return <div className="development-catalog"><section><h2>Vertical faders and optional actions</h2><div className="development-fader-grid">
    <VerticalTouchFader label="No buttons" value={levels[0]} onChange={(value) => update(0, value)}/>
    <VerticalTouchFader label="One button" value={levels[1]} actions={[{ id: "set", label: "SET" }]} onChange={(value) => update(1, value)}/>
    <VerticalTouchFader label="Two buttons" value={levels[2]} actions={[{ id: "go", label: "GO" }, { id: "off", label: "OFF" }]} onChange={(value) => update(2, value)}/>
    <VerticalTouchFader label="Three buttons" value={levels[3]} actions={[{ id: "go", label: "GO" }, { id: "off", label: "OFF" }, { id: "flash", label: "FLASH" }]} onChange={(value) => update(3, value)}/>
  </div></section><section><h2>Horizontal fader</h2><HorizontalFaderField label="Level" value={levels[1]} display={`${Math.round(levels[1])}%`} onChange={(value) => update(1, value)}/></section></div>;
}

function ButtonElements() {
  return <div className="development-catalog"><section><h2>Buttons</h2><div className="development-button-grid"><Button>Secondary</Button><Button variant="primary">Primary</Button><Button variant="success">Success</Button><Button variant="warning">Warning</Button><Button variant="danger">Danger</Button><Button disabled>Disabled</Button></div></section></div>;
}

export function DevelopmentWindow({ compact, developmentView }: WindowProps) {
  const [localView, setLocalView] = useState<DevelopmentView>(developmentView ?? "forms");
  const view = compact ? developmentView ?? "forms" : localView;
  const content = <WindowScrollArea><div className="development-window">{view === "forms" ? <FormElements/> : view === "faders" ? <FaderElements/> : <ButtonElements/>}</div></WindowScrollArea>;
  if (compact) return content;
  return <WindowFrame title="Development" settingsTitle="Development Settings" settingsTabs={[{ id: "content", label: "Content", content: <FormLayout labelPlacement="side"><SelectField label="Shown example" value={view} onChange={setLocalView} options={DEVELOPMENT_VIEW_OPTIONS}/></FormLayout> }]}>{content}</WindowFrame>;
}
