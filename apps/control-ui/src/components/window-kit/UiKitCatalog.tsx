import { useState } from "react";
import { Button, CheckboxField, ColorPickerField, FormField, FormLayout, IconPickerField, Input, NumberField, SelectField, SwitchField, TextField } from "../common";
import { ButtonGrid, DataTable, FaderView, GridButton, WindowFrame, WindowScrollArea } from ".";

const rows = [{ id: 1, name: "Front wash", value: "100%" }, { id: 2, name: "Back wash", value: "42%" }];
export function UiKitCatalog() {
  const [active, setActive] = useState(0);
  const [text, setText] = useState("Front wash");
  const [amount, setAmount] = useState(12.5);
  const [enabled, setEnabled] = useState(true);
  const [mode, setMode] = useState("merge");
  const [icon, setIcon] = useState("◇");
  const [color, setColor] = useState("#d98236");
  const [serverUrl, setServerUrl] = useState("http://desk.local:5000");
  return <main className="ui-kit-catalog">
    <h1>ToskLight UI Kit</h1>
    <section className="catalog-window wide"><WindowFrame title="Stage" info={{ primary: "1 selected", secondary: "Tap to select · Shift for range" }} actions={[[{ id: "follow", label: "Follow Preload", active: true, onClick: () => undefined }],[{ id: "select", label: "Select fixtures", active: true, onClick: () => undefined },{ id: "setup", label: "Setup positions", onClick: () => undefined }]]} settingsTabs={[{ id: "settings", label: "Settings", content: <p>Window-specific settings live here.</p> }]} navigation={<><Button className="active">Overview</Button><Button>Details</Button></>} infoSection={<><b>Selected item</b><p>Information sections stay consistent.</p></>} bottom={<div className="catalog-bottom">Additional button pane</div>}><WindowScrollArea><DataTable rows={rows} columns={[{ id: "id", header: "ID", width: "60px", render: (row) => row.id },{ id: "name", header: "Name", width: "minmax(180px,1fr)", render: (row) => row.name },{ id: "value", header: "Value", width: "100px", render: (row) => row.value }]} rowKey={(row) => String(row.id)} activeIndex={active} onActiveIndexChange={setActive} selected={(row) => row.id === 2} emptyRows={8} /></WindowScrollArea></WindowFrame></section>
    <section><h2>Button Grid</h2><ButtonGrid>{<><GridButton number="1" primary="Open" secondary="Intensity" icon="◇" state="active"/><GridButton number="2" primary="Blue" secondary="Color" icon="●"/><GridButton number="3" primary="Empty" state="empty"/><GridButton number="4" primary="Disabled" state="disabled"/><GridButton number="5" primary="Store here" state="store-target"/></>}</ButtonGrid></section>
    <section className="catalog-form-section"><h2>Unified Forms</h2><FormLayout labelPlacement="side"><TextField label="Name" clearable value={text} onChange={(event) => setText(event.target.value)}/><NumberField label="Level" allowDecimal step="0.5" value={amount} onChange={(event) => setAmount(Number(event.target.value))}/><NumberField label="Address" showStepButtons={false} value={1}/><SelectField label="Record mode" value={mode} onChange={setMode} options={[{ value: "merge", label: "Merge" }, { value: "overwrite", label: "Overwrite" }]}/><IconPickerField label="Icon" value={icon} onChange={setIcon}/><ColorPickerField label="Color" value={color} onChange={setColor}/><SwitchField label="Enabled switch" checked={enabled} onChange={(event) => setEnabled(event.target.checked)}/><CheckboxField label="Enabled checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)}/><FormField label="Arbitrary content"><Button variant="primary">Form action</Button></FormField></FormLayout><h3>Responsive columns with top labels</h3><FormLayout columns={3} minColumnWidth={180}><TextField label="Column one" value="A" readOnly/><TextField label="Column two" value="B" readOnly/><TextField label="Column three" error="Example error" value="C" readOnly/></FormLayout></section>
    <section className="catalog-connection-section"><h2>Connection Form</h2><div className="connection-card"><form className="connection-form"><TextField label="Server" clearable value={serverUrl} onChange={(event) => setServerUrl(event.target.value)}/><div className="connection-form-actions"><Button>Use server</Button></div></form></div></section>
    <section><h2>Fader View</h2><FaderView rows={2}>{Array.from({ length: 8 }, (_, index) => <div className="catalog-fader" key={index}><b>CH {index + 1}</b><Input aria-label={`Channel ${index + 1}`} type="range" defaultValue={(index + 1) * 10}/></div>)}</FaderView></section>
  </main>;
}
