import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SearchBar, TouchSelect } from "./index";
import {
  Button,
  CheckboxField,
  ColorPickerField,
  FileDropField,
  FormField,
  FormLayout,
  GroupedSelectionField,
  IconPickerField,
  LargeTextField,
  MultiValueToggleField,
  NumberField,
  Select,
  SelectField,
  SwitchField,
  TextAreaField,
  TextField,
} from "../controls";
import { HorizontalFaderField } from "./FaderControls";

const meta = {
  title: "Controls/Production controls",
  component: Button,
  tags: ["autodocs"],
  parameters: {
    layout: "fullscreen",
    docs: { source: { type: "dynamic" } },
  },
  argTypes: {
    variant: {
      control: "select",
      options: ["primary", "secondary", "ghost", "danger", "success", "warning"],
    },
    size: { control: "inline-radio", options: ["default", "compact"] },
    active: { control: "boolean" },
    loading: { control: "boolean" },
    disabled: { control: "boolean" },
    fullWidth: { control: "boolean" },
    iconOnly: { control: "boolean" },
  },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ButtonPlayground: Story = {
  args: {
    children: "Apply",
    variant: "secondary",
    size: "default",
    active: false,
    loading: false,
    disabled: false,
    fullWidth: false,
    iconOnly: false,
  },
  render: (args) => <Button {...args} />,
};

export const Buttons: Story = {
  render: () => (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(4, minmax(0, 1fr))", gap: 10 }}>
      <Button variant="primary">Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="ghost">Ghost</Button>
      <Button variant="danger">Danger</Button>
      <Button variant="success">Success</Button>
      <Button variant="warning">Warning</Button>
      <Button active>Active</Button>
      <Button disabled>Disabled</Button>
      <Button loading>Save</Button>
      <Button size="compact">Compact</Button>
      <Button iconOnly aria-label="Settings">⚙</Button>
      <Button fullWidth>Full width</Button>
    </div>
  ),
};

function FormsExample() {
  const [name, setName] = useState("Main Stage");
  const [password, setPassword] = useState("operator");
  const [rows, setRows] = useState("4");
  const [scale, setScale] = useState("1.25");
  const [mode, setMode] = useState("software");
  const [stageView, setStageView] = useState("2d");
  const [level, setLevel] = useState(68);
  const [fullscreen, setFullscreen] = useState(true);
  const [locked, setLocked] = useState(false);
  const [icon, setIcon] = useState("◇");
  const [color, setColor] = useState("#1bd6ec");
  const [buttonAction, setButtonAction] = useState("go");
  const [faderAction, setFaderAction] = useState("master");
  const [fileState, setFileState] = useState("No file selected");
  const notes = [
    "House opens at 18:30.",
    "Preset the front wash at 42%.",
    "Keep the lectern special isolated.",
    "Check the stage-right practical.",
    "Confirm haze with venue staff.",
    "Hold audience blinders until finale.",
    "Followspot channel is intercom 3.",
    "Interval state uses cue 27.",
    "Emergency look is playback 10.",
    "Touring console receives universe 4.",
    "Save a revision after sound check.",
    "Operator handover begins at 19:15.",
  ].join("\n");
  const selectionGroups = [
    { label: "Step Control", options: [
      { value: "go", label: "GO", icon: "▶", description: "Advance to the next cue." },
      { value: "go-minus", label: "GO MINUS", description: "Return to the previous cue." },
    ] },
    { label: "Temporary State", options: [
      { value: "flash", label: "FLASH", icon: "⚡", description: "Output while the button is held." },
    ] },
  ] as const;
  return (
    <div className="forms-story-canvas">
      <section>
        <h2>Top labels</h2>
        <FormLayout columns={2} labelPlacement="top">
          <TextField label="Name" description="Visible operator name" required clearable value={name} onChange={(event) => setName(event.target.value)} />
          <TextField label="Password" secure value={password} onChange={(event) => setPassword(event.target.value)} />
          <NumberField label="Rows" value={rows} min={1} max={18} onValueChange={setRows} />
          <NumberField label="Scale" value={scale} allowDecimal step={0.05} unit="×" onValueChange={setScale} />
          <NumberField label="Fixed value" value="512" showStepButtons={false} disabled />
          <SelectField label="Mode" value={mode} options={[
            { value: "software", label: "Software" },
            { value: "hardware", label: "Hardware" },
            { value: "unavailable", label: "Unavailable", disabled: true },
          ]} onChange={setMode} />
          <MultiValueToggleField label="Stage view" value={stageView} options={[
            { value: "2d", label: "2D" },
            { value: "3d", label: "3D" },
          ]} onChange={setStageView} />
          <SwitchField label="Fullscreen" checked={fullscreen} onChange={(event) => setFullscreen(event.target.checked)} />
          <CheckboxField label="Lock desktop" checked={locked} onChange={(event) => setLocked(event.target.checked)} />
          <IconPickerField label="Icon" value={icon} defaultGroup="gobo" onChange={setIcon} />
          <ColorPickerField label="Color" value={color} onChange={setColor} />
          <TextAreaField label="Notes" defaultValue={notes} placeholder="Add operator notes" />
          <LargeTextField label="Large text" defaultValue={"Line one\nLine two\nLine three"} />
          <TextField label="Required field" error="Required" required />
        </FormLayout>
      </section>
      <section>
        <h2>Side labels and compatibility controls</h2>
        <FormLayout columns={2} labelPlacement="side" labelWidth={130}>
          <TextField label="Compact name" controlSize="compact" defaultValue="Front Wash" />
          <FormField label="Native select">
            <Select aria-label="Universe" defaultValue="1">
              <option value="1">Universe 1</option>
              <option value="2">Universe 2</option>
            </Select>
          </FormField>
          <FileDropField label="Fixture profile"
            constraints={{ extensions: [".gdtf", ".toskfixture"], mimeTypes: ["application/zip"], multiple: false }}
            selectedLabel={fileState}
            onFiles={(files) => setFileState(files[0]?.name ?? "No file selected")}
            onRejected={setFileState}
            onOpenPicker={() => setFileState("ToskLight File Manager opened")} />
          <HorizontalFaderField label="Level" value={level} display={`${level}%`}
            onChange={setLevel} />
        </FormLayout>
      </section>
      <section>
        <h2>Grouped selections</h2>
        <FormLayout columns={2}>
          <GroupedSelectionField label="Top button" value={buttonAction}
            groups={selectionGroups} onChange={setButtonAction}
            clearAction={{ label: "Empty Button", value: "none" }} />
          <GroupedSelectionField label="Fader" value={faderAction}
            groups={[{ label: "Level Control", options: [
              { value: "master", label: "Master", description: "Control playback intensity." },
              { value: "temp", label: "Temp", description: "Temporarily bring the playback in." },
            ] }]} onChange={setFaderAction} />
        </FormLayout>
      </section>
      <section>
        <h2>File drop states</h2>
        <FormLayout columns={3}>
          <FileDropField label="Loading" status="loading" statusMessage="Loading selected file…"
            constraints={{ extensions: [".gdtf"] }} onFiles={() => undefined} onOpenPicker={() => undefined} />
          <FileDropField label="Success" status="success" statusMessage="touring-profile.gdtf"
            constraints={{ extensions: [".gdtf"] }} onFiles={() => undefined} onOpenPicker={() => undefined} />
          <FileDropField label="Actionable error" status="error" statusMessage="The archive could not be read."
            constraints={{ extensions: [".gdtf"] }} onFiles={() => undefined} onOpenPicker={() => undefined} />
        </FormLayout>
      </section>
    </div>
  );
}

export const Forms: Story = {
  render: () => <FormsExample />,
};

function SearchAndSelectExample() {
  const [search, setSearch] = useState("wash");
  const [universe, setUniverse] = useState(1);
  const [filter, setFilter] = useState("");
  return (
    <div style={{ width: 720, display: "grid", gap: 16 }}>
      <SearchBar value={search} onChange={setSearch} />
      <SearchBar
        value={search}
        onChange={setSearch}
        settings={[{
          kind: "select",
          id: "type",
          label: "Fixture type",
          value: filter,
          options: [
            { value: "", label: "All" },
            { value: "Dimmer", label: "Dimmer" },
            { value: "Moving light", label: "Moving light" },
          ],
        }]}
        onSettingChange={(_, value) => setFilter(String(value))}
        onClearSettings={() => setFilter("")}
      />
      <TouchSelect
        label="Universe"
        value={universe}
        options={[1, 2, 3, 4]}
        onChange={setUniverse}
      />
    </div>
  );
}

export const SearchAndTouchSelect: Story = {
  render: () => <SearchAndSelectExample />,
};
