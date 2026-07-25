const expectationWords = new Set([
	"absent",
	"active",
	"button",
	"configuration",
	"dirty",
	"empty",
	"fixtures",
	"metadata",
	"named",
	"present",
	"ready",
	"revision",
	"runtime",
	"selected",
	"unpatched",
]);

const worldFamilies = {
	app: ["Browser UI"],
	builtIn: ["Browser UI"],
	clock: ["Test clock"],
	command: ["Command line"],
	cue: ["Cue runtime"],
	demo: ["Browser UI", "Generated artifacts"],
	desktop: ["Browser UI", "Desktop layout"],
	dmx: ["DMX output"],
	encoder: ["Programmer", "Encoder controls"],
	expect: ["Semantic oracle"],
	expectFixtureDMX: ["DMX output"],
	expectFixtureDMXAbsent: ["DMX output"],
	expectFixtureValue: ["Resolved output"],
	group: ["Groups"],
	hardware: ["Attached hardware", "OSC"],
	highlight: ["Highlight"],
	keypad: ["Command keypad"],
	output: ["Output runtime"],
	page: ["Playback pages"],
	patch: ["Show Patch"],
	playback: ["Playbacks"],
	preload: ["Preload"],
	preset: ["Presets"],
	programmer: ["Programmer"],
	record: ["Recording"],
	recipe: ["Test recipe"],
	screen: ["Secondary screen"],
	screenshot: ["Generated artifacts"],
	selection: ["Selection"],
	show: ["Show files"],
	special: ["Programmer"],
	speedGroup: ["Speed Groups"],
	timing: ["Programmer timing", "Cue timing"],
};

const returnedFamilies = {
	desktopBuilder: ["Browser UI", "Desktop layout"],
	screenHandle: ["Secondary screen"],
};

// Public calls used by marked scenarios are opt-in. A new helper path must be
// reviewed here so generic prose never silently invents its meaning.
const supportedCallPaths = new Set([
	"app.expect.ready",
	"app.open",
	"builtIn.open",
	"clock.advanceBy",
	"clock.advanceStep",
	"clock.at",
	"command.execute",
	"command.clear",
	"command.expect",
	"command.type",
	"cue.expect.metadata",
	"cue.expect.absent",
	"cue.expect.groupValue",
	"cue.expect.present",
	"cue.expect.trigger",
	"cue.expect.groupValueTiming",
	"cue.configure",
	"cue.goto",
	"demo.run",
	"desktop.closeSettings",
	"desktop.configure",
	"desktop.create",
	"desktop.openSettingsFor",
	"desktopBuilder.addPane",
	"desktopBuilder.apply",
	"encoder.beam.gobo1.via.api.set",
	"encoder.clear",
	"encoder.color.blue.via.api.set",
	"encoder.color.green.via.api.set",
	"encoder.color.red.via.api.set",
	"encoder.intensity.dimmer.release",
	"encoder.intensity.dimmer.via.api.add",
	"encoder.intensity.dimmer.via.api.set",
	"encoder.intensity.dimmer.via.api.subtract",
	"encoder.intensity.dimmer.via.ui.set",
	"encoder.position.pan.via.api.set",
	"expect.rejects.toThrow",
	"expect.selection",
	"expect.toBe",
	"expect.toEqual",
	"expect.toMatchObject",
	"expectFixtureDMX",
	"expectFixtureDMXAbsent",
	"expectFixtureValue",
	"group.expect.absent",
	"group.expect.empty",
	"group.expect.fixtures",
	"group.expect.metadata",
	"group.expect.present",
	"group.via.api.delete",
	"group.via.api.select",
	"group.via.api.store",
	"group.via.keypad.store",
	"group.via.pool.edit",
	"group.via.pool.select",
	"group.via.pool.store",
	"hardware.connect",
	"hardware.disconnect",
	"highlight.via.api.off",
	"highlight.via.api.on",
	"highlight.via.osc.toggle",
	"keypad.press",
	"page.create",
	"page.expect.named",
	"page.expect.selected",
	"page.map",
	"page.rename",
	"page.via.api.select",
	"page.via.ui.select",
	"patch.expect.unpatched",
	"patch.via.ui.unpatch",
	"playback.configure",
	"playback.expect.configuration",
	"playback.expect.present",
	"playback.expect.runtime",
	"playback.expect.selected",
	"playback.go",
	"playback.goBack",
	"playback.off",
	"playback.on",
	"playback.pause",
	"playback.release",
	"playback.select",
	"playback.via.api.go",
	"playback.via.api.off",
	"playback.via.api.on",
	"playback.via.api.select",
	"playback.via.ui.flash.hold",
	"playback.via.ui.go",
	"playback.via.ui.temp",
	"preload.release",
	"preload.start",
	"preset.expect.absent",
	"preset.expect.button",
	"preset.expect.metadata",
	"preset.expect.present",
	"preset.recall",
	"preset.via.api.store",
	"preset.via.keypad.delete",
	"preset.via.keypad.store",
	"preset.via.osc.recall",
	"preset.via.pool.edit",
	"preset.via.pool.recall",
	"preset.via.pool.store",
	"programmer.priority.via.api.set",
	"record.append",
	"record.cue",
	"record.cueOnly",
	"record.playback",
	"record.via.api.cue",
	"record.via.ui.playback",
	"screen.create",
	"screenHandle.close",
	"screenHandle.expectBridgeAction",
	"screenHandle.page.expectSelected",
	"screenHandle.page.select",
	"screenHandle.remove",
	"screenshot.application",
	"screenshot.builtIn",
	"screenshot.dialog",
	"selection.clear",
	"selection.fixtures.via.api.item",
	"selection.fixtures.via.api.items",
	"selection.fixtures.via.fixtureSheet.item",
	"selection.fixtures.via.fixtureSheet.items",
	"selection.fixtures.via.fixtureSheet.range",
	"selection.fixtures.via.keypad.range",
	"selection.fixtures.via.osc.item",
	"selection.fixtures.via.stage.range",
	"selection.groups.range",
	"selection.groups.via.pool.range",
	"selection.observe",
	"selection.targets",
	"show.create",
	"show.expect.active",
	"show.expect.dirty",
	"show.expect.revision",
	"show.load",
	"show.loadRevision",
	"show.resetWorkingCopy",
	"show.save",
	"show.saveAs",
	"show.saveRevision",
	"show.use",
	"show.via.ui.loadRevision",
	"show.via.ui.saveAs",
	"show.via.ui.saveRevision",
	"timing.cueFade.set",
	"timing.programmerFade.via.api.set",
]);

const exactNarrations = new Map([
	[
		"app.open",
		([target]) =>
			`Open the ToskLight browser application${target ? ` at ${target}` : ""}.`,
	],
	[
		"app.expect.ready",
		() => "Expect the ToskLight browser application to be ready.",
	],
	["show.use", ([show]) => `Use the isolated ${show} show.`],
	["show.create", ([name]) => `Create the show ${name}.`],
	["show.load", ([show]) => `Load the show ${show}.`],
	["show.save", () => "Save the active show."],
	["show.saveAs", ([name]) => `Save the active show as ${name}.`],
	["show.saveRevision", ([name]) => `Save a named show revision ${name}.`],
	[
		"show.loadRevision",
		([show, revision]) => `Load revision ${revision} of ${show}.`,
	],
	["show.resetWorkingCopy", () => "Reset the isolated working copy."],
	["command.execute", ([command]) => `Execute the desk command ${command}.`],
	["command.expect", ([value]) => `Expect the command line to show ${value}.`],
	["keypad.press", ([keys]) => `Press keypad keys ${keys}.`],
	["clock.advanceBy", ([duration]) => `Advance the test clock by ${duration}.`],
	[
		"clock.advanceStep",
		() => "Advance the test clock by one deterministic step.",
	],
	[
		"expectFixtureDMX",
		([target, expected]) => `Expect ${target} DMX to equal ${expected}.`,
	],
	[
		"expectFixtureDMXAbsent",
		([target]) => `Expect ${target} to have no DMX output.`,
	],
	[
		"expectFixtureValue",
		([target, expected]) =>
			`Expect ${target} resolved values to equal ${expected}.`,
	],
	[
		"demo.run",
		() =>
			"Run the complete narrated Full HD product-demo workflow and its internal semantic assertions.",
	],
	[
		"hardware.connect",
		() => "Connect the simulated attached hardware surface.",
	],
	[
		"hardware.disconnect",
		() => "Disconnect the simulated attached hardware surface.",
	],
	["selection.clear", () => "Clear the current selection."],
	[
		"encoder.clear",
		() => "Clear Programmer values through the encoder intent.",
	],
	["preload.start", () => "Start blind Preload programming."],
	["preload.release", () => "Release blind Preload programming."],
	[
		"desktopBuilder.addPane",
		([type, geometry]) => `Add the ${type} pane with layout ${geometry}.`,
	],
	["desktopBuilder.apply", () => "Apply the configured Desktop layout."],
	["screenHandle.open", () => "Open the configured secondary screen."],
	["screenHandle.close", () => "Close the configured secondary screen."],
	["screenHandle.remove", () => "Remove the configured secondary screen."],
	[
		"screenHandle.expectBridgeAction",
		([action]) => `Expect the secondary-screen bridge action ${action}.`,
	],
]);

const implicitOutcomes = new Map([
	[
		"demo.run",
		"Expect the complete product-demo workflow and its internal semantic assertions to pass.",
	],
]);

const routeSurfaces = new Map([
	["api", "Typed API"],
	["click", "Browser UI"],
	["fixtureSheet", "Fixture Sheet"],
	["keypad", "Command keypad"],
	["osc", "OSC"],
	["pool", "Pool UI"],
	["shiftClick", "Browser UI"],
	["stage", "Stage"],
	["touch", "Touch UI"],
	["ui", "Browser UI"],
]);

export function narrateCall(call) {
	if (!supportedCallPaths.has(call.path)) return undefined;
	const family = call.path.split(".")[0];
	const familySurfaces = worldFamilies[family] ?? returnedFamilies[family];
	if (!familySurfaces) return undefined;

	const explicit = exactNarrations.get(call.path);
	const kind = expectationKind(call.path, call.root);
	const description = explicit
		? explicit(call.arguments)
		: genericNarration(call.path, call.arguments, kind);
	const surfaces = new Set(familySurfaces);
	for (const segment of call.path.split(".")) {
		const route = routeSurfaces.get(segment);
		if (route) surfaces.add(route);
	}
	return {
		kind,
		description,
		surfaces: [...surfaces].sort(),
		resultType: resultTypeFor(call.path),
		expectedOutcome: implicitOutcomes.get(call.path),
	};
}

function expectationKind(callPath, root) {
	if (root === "expect") return "expected-outcome";
	if (
		callPath.startsWith("expect.") ||
		callPath.startsWith("expectFixture") ||
		callPath.includes(".expect.") ||
		callPath.split(".").at(-1).startsWith("expect")
	)
		return "expected-outcome";
	const terminal = callPath.split(".").at(-1);
	return expectationWords.has(terminal) && callPath.includes("expect")
		? "expected-outcome"
		: "step";
}

function genericNarration(callPath, args, kind) {
	const words = callPath
		.split(".")
		.filter((part) => part !== "via")
		.map(humanize)
		.join(" → ");
	const suffix = args.length ? ` with ${args.join(", ")}` : "";
	return `${kind === "expected-outcome" ? "Expect" : "Perform"} ${words}${suffix}.`;
}

function humanize(value) {
	return value
		.replace(/([a-z0-9])([A-Z])/gu, "$1 $2")
		.replaceAll("_", " ")
		.toLowerCase();
}

function resultTypeFor(callPath) {
	if (callPath === "desktop.configure") return "desktopBuilder";
	if (callPath === "screen.create") return "screenHandle";
	return undefined;
}

export function catalogFamilies() {
	return {
		world: Object.keys(worldFamilies).sort(),
		returned: Object.keys(returnedFamilies).sort(),
	};
}
