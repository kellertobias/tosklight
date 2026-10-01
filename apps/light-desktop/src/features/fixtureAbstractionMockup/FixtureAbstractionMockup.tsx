import { Button, ModalFrame } from "@tosklight/ui";
import { EncoderSection, type EncoderSectionItem } from "@tosklight/ui/encoders";
import { type ReactNode, useEffect, useState } from "react";
import { ParameterFamilyTabs } from "../../components/control/parameterControls/ParameterFamilyTabs";
import type { ParameterFamily } from "../../components/control/parameterControls/model";
import type { ParameterController } from "../../components/control/parameterControls/useParameterController";
import { VisibleEncoderCountProvider } from "../../components/control/parameterControls/VisibleEncoderCount";
import { AppShellView } from "../../components/shell/AppShell";
import { LeftDock } from "../../components/shell/LeftDock";
import { useApp } from "../../state/AppContext";
import { FixtureConfigurationMockup } from "./FixtureConfigurationMockup";
import { MediaPreview } from "./MediaPreview";
import { mockupEncoder, type MockupSurface } from "./MockupControls";
import { initialRecipe, points, recipePreview, resolveAngles, resolveTarget, wheelColors, wheelCorrections, type PositionIntent, type Recipe, type Vec3 } from "./mockupModel";
import { ProgrammingWorkspace } from "./ProgrammingWorkspace";
import { useEncoderArea } from "./ProgrammingEditor";
import { ColorMatches, ExampleColorMatches, magentaRecipe, warmWhiteRecipe } from "./ColorMatches";
import { ColorPlane } from "./ColorPlane";
import { ColorProgrammingEditor } from "./ColorProgrammingEditor";
import { HueRing, RangeFader, recipeHsv, spreadRecipes, type ColorRangeKey, type ColorRanges, type ValueRange } from "./RangeControls";
import { hsvToRgb } from "../../components/modals/specialColor";
import { PositionProgrammingEditor } from "./PositionProgrammingEditor";
import { FocusProgrammingEditor } from "./FocusProgrammingEditor";
import "./fixtureAbstractionMockup.css";

export type MockupView = "easy" | "extended" | "advanced" | "mixed-magenta" | "mixed-warm-white" | "angles" | "fixed" | "tracked" | "configuration" | "beam" | "media";
export interface FixtureAbstractionMockupProps {
 initialView?: MockupView; surface?: MockupSurface;
 colorMode?: "easy" | "advanced"; easyLayout?: "rgbw" | "rgbwauv";
 fixtureCapability?: "mixed" | "rgbw" | "rgbwauv" | "rgbal" | "cmy" | "wheel";
 mountLift?: number; mountYaw?: number; performerOffset?: number;
}

/** Supplies the desk's control section around the mockup's programmer area. */
export type MockupControlRenderer = (control: { hardware: boolean; programmer: ReactNode }) => ReactNode;

/**
 * Local illustration composed inside the production shell; no show or hardware transport.
 * The caller provides application state, show objects and the control section.
 */
export function FixtureAbstractionMockup(props: FixtureAbstractionMockupProps & { renderControl: MockupControlRenderer }) {
	return <VisibleEncoderCountProvider count={4}>
		<MockupDesk {...props} />
	</VisibleEncoderCountProvider>;
}

function MockupDesk({ initialView = "easy", surface = "touch", colorMode, easyLayout, fixtureCapability, mountLift = 0, mountYaw = 0, performerOffset = 0, renderControl }: FixtureAbstractionMockupProps & { renderControl: MockupControlRenderer }) {
	const { state } = useApp();
	const positionView = ["angles", "fixed", "tracked"].includes(initialView);
	const media = initialView === "media";
	const mixedExample = initialView.startsWith("mixed-");
	const [family, setFamily] = useState<ParameterFamily>(positionView ? "Position" : initialView === "beam" ? "Focus" : "Color");
	const [page, setPage] = useState(initialView === "fixed" || initialView === "tracked" ? 2 : 1);
	const [dialog, setDialog] = useState(initialView === "advanced" || mixedExample);
	const [editorPage, setEditorPage] = useState(initialView === "mixed-warm-white" ? "white" : "mix");
	const area = useEncoderArea();
	const mode = colorMode ?? (initialView === "advanced" || mixedExample ? "advanced" : "easy");
	const layout = easyLayout ?? (initialView === "extended" ? "rgbwauv" : "rgbw");
	const fixture = fixtureCapability ?? (mixedExample ? "mixed" : initialView === "advanced" ? "cmy" : initialView === "extended" ? "rgbwauv" : "rgbw");
	const [configuration, setConfiguration] = useState(initialView === "configuration");
	const [recipe, setRecipe] = useState<Recipe>(initialView === "mixed-magenta" ? magentaRecipe : initialView === "mixed-warm-white" ? warmWhiteRecipe : media ? { ...initialRecipe, red: 100, green: 100, blue: 100 } : initialRecipe);
	const [colorHue, setColorHue] = useState(() => recipeHsv(recipe).hue);
	const [intensity, setIntensity] = useState(100);
	const [beam, setBeam] = useState({ zoom: 24, iris: 100, strobe: 8, zoomPulse: 0, irisPulse: 0 });
	const [focus, setFocus] = useState({ focus: 50, frost: 0 });
	const [shutter, setShutter] = useState("open");
	// One active Position group. Point/offset below are editor drafts, never parallel outputs.
	const [intent, setIntent] = useState<PositionIntent>(initialView === "fixed"
		? { type: "target", point: "origin", offset: { x: 0, y: 2, z: 0 } }
		: initialView === "tracked" ? { type: "target", point: "point-stage-center", offset: { x: 0, y: 0, z: 0 } }
		: { type: "angles", pan: 24, tilt: 42 });
	const [point, setPoint] = useState(initialView === "tracked" ? "point-stage-center" : "origin");
	const [offset, setOffset] = useState<Vec3>(initialView === "fixed" ? { x: 0, y: 2, z: 0 } : { x: 0, y: 0, z: 0 });
	const [ranges, setRanges] = useState<ColorRanges>({});
	useEffect(() => { if (family === "Color") setPage(1); }, [mode, layout]);
	const lift = mountLift, yaw = mountYaw, performer = performerOffset;
	const mount = { x: -2, y: -2, z: 5 + lift };
	const target = intent.type === "target" ? resolveTarget(intent.point, intent.offset, lift, performer) : null;
	const angles = target ? resolveAngles(target, mount, yaw) : intent.type === "angles" ? intent : { pan: 0, tilt: 0 };
	const updateRecipe = (key: string, value: number) => {
		if (["red", "green", "blue"].includes(key)) { const next = recipeHsv({ ...recipe, [key]: value }); if (next.saturation > 0) setColorHue(next.hue); }
		setRecipe(current => ({ ...current, [key]: value }));
		setRanges(current => { const next = { ...current }; delete next[key as ColorRangeKey]; if (["red", "green", "blue"].includes(key)) { delete next.hue; delete next.saturation; } return next; });
	};
	const applyColor = (key: ColorRangeKey, value: number, range?: ValueRange) => {
		if (key === "hue") setColorHue(value);
		setRanges(current => ({ ...current, [key]: range }));
		setRecipe(current => {
			if (key !== "hue" && key !== "saturation") return { ...current, [key]: value };
			const hsv = recipeHsv(current, colorHue);
			const rgb = hsvToRgb({ hue: (key === "hue" ? value : hsv.hue) / 360, saturation: (key === "saturation" ? value : hsv.saturation) / 100, brightness: hsv.brightness || 1 });
			return { ...current, red: rgb[0] * 100, green: rgb[1] * 100, blue: rgb[2] * 100, amber: 0 };
		});
	};
	const adoptTarget = (nextPoint: string, nextOffset: Vec3) => {
		setPoint(nextPoint); setOffset(nextOffset); setIntent({ type: "target", point: nextPoint, offset: { ...nextOffset } });
	};
	const update = (key: string, value: number) => {
		if (key === "pan" || key === "tilt") setIntent({ type: "angles", pan: angles.pan, tilt: angles.tilt, [key]: value });
		else if (["x", "y", "z"].includes(key)) adoptTarget(point, { ...offset, [key]: value });
		else if (key === "point") adoptTarget(points[Math.round(value)].id, offset);
		else if (key === "intensity") setIntensity(value);
		else if (key === "shutter") setShutter(["open", "closed", "regular", "random-slow", "random-fast"][Math.round(value)]);
		else if (key in focus) setFocus(current => ({ ...current, [key]: value }));
		else if (key in beam) setBeam(current => ({ ...current, [key]: value }));
		else updateRecipe(key, value);
	};
	const numeric = (key: string, label: string, value: number, slot: number, unit = "%", min = 0, max = 100, step = 1) => {
		const item = mockupEncoder(key, label, value, slot, unit, min, max, step);
		if (area.narrow && ["m", "Duv", "K", "Hz"].includes(unit) && item.target) {
			item.target.display = String(Number(item.target.display.replace(` ${unit}`, "")));
			if (unit === "Duv") item.target.display = item.target.display.replace(/^(-?)0\./, "$1.");
			item.mode = unit;
			item.target.label = `${key === "temperature" ? "Temp" : key === "tint" ? "Tint" : label} (${unit})`;
		}
		return item;
	};
	const choice = (key: string, label: string, value: number, slot: number, names: string[]): EncoderSectionItem => ({
		...numeric(key, label, value, slot, "", 0, names.length - 1),
		touchInteraction: "choices", target: { label, display: names[value] },
		presets: { selectedValue: String(value), groups: [{ label, options: names.map((name, index) => ({ value: String(index), label: name })) }] },
	});
	let items: EncoderSectionItem[];
	if (family === "Position") items = page === 1 ? [numeric("pan", "Pan", angles.pan, 1, "°", -720, 720), numeric("tilt", "Tilt", angles.tilt, 2, "°", -135, 135)]
		: [choice("point", "Point", points.findIndex(p => p.id === point), 1, points.map(p => p.name)), ...(["x", "y", "z"] as const).map((axis, index) => ({ ...numeric(axis, axis.toUpperCase(), offset[axis], index + 2, "m", -100, 100, .1), mode: area.narrow ? "m" : point === "origin" ? "World coordinate · m" : "Point offset · m" }))];
	else if (family === "Color") items = page === 1 ? [numeric("red", "Red", recipe.red, 1), numeric("green", "Green", recipe.green, 2), numeric("blue", "Blue", recipe.blue, 3), {
		...numeric("white", "White Blend", recipe.white, 4), presets: { selectedValue: String(recipe.white), groups: [{ label: "White Blend", options: [0, 50, 100].map(v => ({ value: String(v), label: `${v}%` })) }] },
	}] : mode === "advanced" ? [numeric("temperature", "Temperature", recipe.temperature, 1, "K", 1000, 20000, 100), numeric("tint", "Green / Magenta", recipe.tint, 2, "Duv", -.03, .03, .0005), choice("wheel1", "Wheel 1", recipe.wheel1, 3, wheelColors), choice("wheel2", "Wheel 2", recipe.wheel2, 4, wheelCorrections)]
		: [numeric("amber", "Amber", recipe.amber, 1), numeric("uv", "UV", recipe.uv, 2)];
	else if (family === "Focus") items = [numeric("zoom", "Zoom", beam.zoom, 1, "°", 8, 48), numeric("focus", "Focus", focus.focus, 2), numeric("frost", "Frost", focus.frost, 3)];
	else if (family === "Beam") items = [numeric("iris", "Iris", beam.iris, 1), numeric("zoomPulse", "Zoom pulse", beam.zoomPulse, 2, "Hz", 0, 10, .1), numeric("irisPulse", "Iris pulse", beam.irisPulse, 3, "Hz", 0, 10, .1)];
	else if (family === "Intensity") items = [numeric("intensity", "Intensity", intensity, 1), { ...choice("shutter", "Shutter", ["open", "closed", "regular", "random-slow", "random-fast"].indexOf(shutter), 2, ["Open", "Closed", "Regular strobe", "Random slow", "Random fast"]), mode: "Shutter behavior" }, numeric("strobe", "Strobe", beam.strobe, 3, "Hz", .5, 30, .5)];
	else items = [];
	while (items.length < 4) items.push({ id: `unused-${items.length}`, slot: items.length + 1, value: 0, disabled: true });
	const onPreset = (id: string, value: string) => {
		if (id === "shutter") setShutter(["open", "closed", "regular", "random-slow", "random-fast"][Number(value)]);
		else update(id, Number(value));
	};
	const controller = {
		state, family, encoderPage: page, alignMode: null, dynamicsMode: false,
		dispatch: () => { if (family === "Color") { if (dialog) setEditorPage(current => current === "mix" ? "white" : "mix"); else { setDialog(true); setEditorPage("mix"); } } else if (family === "Position") { setDialog(current => !current); setEditorPage(page === 1 ? "angles" : "target"); } else if (family === "Focus") setDialog(current => !current); },
		encoderGroups: ["Intensity", "Color", "Position", "Beam", "Shapers", "Focus", "Control", "Media"].map(name => ({ id: name.toLowerCase(), pages: Array.from({ length: name === "Position" || name === "Color" && !media && (mode === "advanced" || layout === "rgbwauv") ? 2 : 1 }, (_, index) => ({ number: index + 1, slots: [] })) })),
		selectEncoderGroup: (next: ParameterFamily, nextPage: number) => { if (dialog && next === family) { setDialog(false); return; } setFamily(next); setPage(nextPage); setDialog(false); setEditorPage("mix"); },
		setAlignMode: () => {}, setDynamicsMode: () => {}, selectedFixtures: [],
	} as unknown as ParameterController;
	const isPosition = family === "Position", mixed = fixture === "mixed";
	const hsv = recipeHsv(recipe, colorHue);
	const selectionRecipes = spreadRecipes(recipe, ranges, mixed ? 3 : 4, colorHue);
	const shifted = state.shiftArmed;
	const whiteBlend = <RangeFader gradient={media ? "linear-gradient(90deg, #303842, #fff)" : `linear-gradient(90deg, ${recipePreview({ ...recipe, white: 0 }).hex}, ${recipePreview({ ...recipe, white: 50 }).hex} 50%, ${recipePreview({ ...recipe, white: 100 }).hex})`} label="White Blend" value={recipe.white} range={ranges.white} shiftArmed={shifted} onChange={(v, r) => applyColor("white", v, r)} />;
	const whiteBalance = <div className="fam-white-balance">
		<RangeFader label="Temperature" value={recipe.temperature} range={ranges.temperature} min={1000} max={20000} step={100} format={v => `${Math.round(v)} K`} gradient="linear-gradient(90deg, #ff902b, #fff 50%, #75aaff)" shiftArmed={shifted} onChange={(v, r) => applyColor("temperature", v, r)} />
		<RangeFader label="Duv" value={recipe.tint} range={ranges.tint} min={-.03} max={.03} step={.0005} format={v => `${v > 0 ? "+" : ""}${v.toFixed(4)}`} gradient="linear-gradient(90deg, #f783e5, #fff 50%, #77da91)" shiftArmed={shifted} onChange={(v, r) => applyColor("tint", v, r)} />
	</div>;
	const pickerPreview = media ? recipePreview(recipe).baseHex : recipePreview(recipe).hex;
	const colorPicker = <ColorPlane hue={hsv.hue} saturation={hsv.saturation} hueRange={ranges.hue} saturationRange={ranges.saturation} preview={pickerPreview} shiftArmed={shifted}
		onChange={(hue, saturation, hueRange, saturationRange) => {
			setColorHue(hue); setRanges(current => ({ ...current, hue: hueRange, saturation: saturationRange }));
			setRecipe(current => { const rgb = hsvToRgb({ hue: hue / 360, saturation: saturation / 100, brightness: recipeHsv(current).brightness || 1 }); return { ...current, red: rgb[0] * 100, green: rgb[1] * 100, blue: rgb[2] * 100, amber: 0 }; });
		}} />;
	const expandedPicker = <HueRing preview={pickerPreview} hue={hsv.hue} range={ranges.hue} saturation={hsv.saturation} saturationRange={ranges.saturation} shiftArmed={shifted} onHue={(v, r) => applyColor("hue", v, r)} onSaturation={(v, r) => applyColor("saturation", v, r)} controls={<>{whiteBlend}{!media && whiteBalance}</>} />;
	const closeEditor = () => { setDialog(false); area.ref.current?.parentElement?.querySelector<HTMLButtonElement>(".special-dialogs")?.focus(); };
	const comparison = mixed ? <ColorMatches recipe={recipe} recipes={selectionRecipes} ranged={Object.values(ranges).some(Boolean)} /> : <ExampleColorMatches recipes={selectionRecipes} capability={fixture} />;
	const editor = isPosition ? <PositionProgrammingEditor fits={area.fits} pan={angles.pan} tilt={angles.tilt} onClose={closeEditor}
		onAngles={(pan, tilt) => setIntent({ type: "angles", pan, tilt })} onGestureEnd={() => setPage(1)} />
		: family === "Focus" ? <FocusProgrammingEditor fits={area.fits} zoom={beam.zoom} focus={focus.focus} onZoom={zoom => setBeam(current => ({ ...current, zoom }))} onFocus={value => setFocus(current => ({ ...current, focus: value }))} onClose={closeEditor} />
		: <ColorProgrammingEditor fits={area.fits} page={editorPage} onPage={setEditorPage} onClose={closeEditor} picker={colorPicker} expandedPicker={expandedPicker} blend={whiteBlend} balance={whiteBalance} comparison={comparison}
			preview={media ? <MediaPreview rgb={[recipe.red, recipe.green, recipe.blue]} whiteBlend={recipe.white} intensity={intensity} /> : undefined} />;
	const programmer = <div className="parameter-controls"><ParameterFamilyTabs controller={controller} specialDialog={family === "Focus" ? <Button className="special-dialogs" aria-label="Special Dialog" onClick={() => setDialog(current => !current)}><span className="special-dialog-label-full"><span>Special</span><span>Dialog</span></span><span className="special-dialog-label-compact">Spcl</span></Button> : undefined} />
		<div className="parameter-surfaces fam-surfaces" ref={area.ref} data-testid="encoder-area">
			{(!dialog || !area.fits || family === "Position" || family === "Focus") && <><EncoderSection showHeader={false} surface={surface} model={{ id: family.toLowerCase(), label: `${family} encoders`, encoders: items }} callbacks={{ onAbsoluteChange: update, onPresetSelect: onPreset, onRelativeChange: (id, delta) => { const item = items.find(i => i.id === id); if (item) update(id, Math.max(item.minimum ?? 0, Math.min(item.maximum ?? 100, item.value + delta))); } }} /></>}

			{dialog && editor}
		</div>
	</div>;
	return <div className="fixture-abstraction-mockup" data-testid="fixture-abstraction-mockup">
		<AppShellView dock={<LeftDock presentation={{ showIdentity: "Festival", clock: <span className="dock-clock">20:15:00</span>, showIndicator: { className: "show-status-connected", connected: true, label: "Festival", detail: "Local review desk" } }} />}
			control={renderControl({ hardware: surface === "hardware", programmer })}
			workspace={<ProgrammingWorkspace mixedSelection={mixed} />} />
		{configuration && <ModalFrame title="Fixture configuration" ariaLabel="Fixture configuration" closeLabel="Close fixture configuration" onClose={() => setConfiguration(false)} dialogClassName="fixture-abstraction-panel fam-configuration-modal" className="fixture-abstraction-layer"><div className="fam-modal-body"><FixtureConfigurationMockup /></div></ModalFrame>}
	</div>;
}
