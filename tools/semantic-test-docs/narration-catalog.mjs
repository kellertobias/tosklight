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
	cueEditor: ["Browser UI", "Cues"],
	cueListSettings: ["Browser UI", "Cuelist Settings"],
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
	"cue.expectList.configuration",
	"cue.configure",
	"cue.delete",
	"cue.goto",
	"cue.openEditor",
	"cue.reopenEditor",
	"cue.transferChoice.cancel",
	"cueEditor.edit",
	"cueEditor.expect.selected",
	"cueEditor.expect.structure",
	"cueEditor.inspectSettings",
	"cueEditor.openSettings",
	"cueEditor.reject",
	"cueEditor.select",
	"cueListSettings.configure",
	"cueListSettings.expectDefaults",
	"cueListSettings.save",
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
	"playback.open",
	"playback.pause",
	"playback.release",
	"playback.select",
	"playback.toggle",
	"playback.via.api.go",
	"playback.via.api.off",
	"playback.via.api.on",
	"playback.via.api.select",
	"playback.via.ui.flash.hold",
	"playback.via.ui.go",
	"playback.via.ui.temp",
	"preload.release",
	"preload.setFixtureValue",
	"preload.start",
	"preload.commit",
	"preload.configure",
	"preload.expect.active",
	"preload.expect.inactive",
	"preload.expect.pendingPlaybackActions",
	"preload.via.api.commit",
	"preload.via.ui.release",
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
	"speedGroup.A.expect.bpm",
	"speedGroup.A.expect.bpmWithin",
	"speedGroup.A.expect.synchronizedFrom",
	"speedGroup.A.via.click.tapTempo",
	"speedGroup.B.expect.bpm",
	"speedGroup.C.expect.bpm",
	"speedGroup.C.expect.synchronizedFrom",
	"speedGroup.D.expect.bpm",
	"speedGroup.E.expect.bpm",
	"timing.cueFade.set",
	"timing.programmerFade.via.api.set",
]);

const exactNarrations = new Map([
	[
		"app.open",
		([target]) =>
			`Open the ToskLight browser application${target ? ` at ${target}` : ""}.`,
	],
	["app.expect.ready", () => "The ToskLight browser application is ready."],
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
	[
		"show.expect.active",
		([show]) => `Show → active is ${describeActiveShowReference(show)}.`,
	],
	["show.expect.dirty", ([dirty]) => `Show → dirty is ${dirty}.`],
	[
		"show.expect.revision",
		([revision]) => `Show → revision matches ${revision}.`,
	],
	["command.execute", ([command]) => `Execute the desk command ${command}.`],
	["command.expect", ([value]) => `Command line shows ${value}.`],
	["expect.toBe", ([actual, expected]) => `${actual} is ${expected}.`],
	["expect.toEqual", ([actual, expected]) => `${actual} equals ${expected}.`],
	[
		"expect.toMatchObject",
		([actual, expected]) => `${actual} matches ${expected}.`,
	],
	[
		"expect.rejects.toThrow",
		([operation, error]) => `Calling ${operation} rejects with ${error}.`,
	],
	["keypad.press", ([keys]) => `Press keypad keys ${keys}.`],
	["clock.advanceBy", ([duration]) => `Advance the test clock by ${duration}.`],
	[
		"clock.advanceStep",
		() => "Advance the test clock by one deterministic step.",
	],
	[
		"expectFixtureDMX",
		([target, expected]) => `${target} DMX equals ${expected}.`,
	],
	["expectFixtureDMXAbsent", ([target]) => `${target} has no DMX output.`],
	[
		"expectFixtureValue",
		([target, expected]) => `${target} resolved values equal ${expected}.`,
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
		([action]) => `Secondary-screen bridge action is ${action}.`,
	],
	[
		"screenHandle.page.expectSelected",
		([page]) => `Secondary screen → selected page is ${page}.`,
	],
	[
		"screenshot.application",
		([name]) => `Capture application screenshot ${name}.`,
	],
	[
		"screenshot.builtIn",
		([pane, name]) => `Capture ${pane} pane screenshot ${name}.`,
	],
	[
		"screenshot.dialog",
		([dialog, name]) => `Capture ${dialog} dialog screenshot ${name}.`,
	],
]);

const implicitOutcomes = new Map([
	[
		"demo.run",
		"Complete product-demo workflow and its internal semantic assertions pass.",
	],
]);

const preconditionCallPaths = new Set([
	"app.expect.ready",
	"app.open",
	"show.create",
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
]);

const toolCallPaths = new Set([
	"screenshot.application",
	"screenshot.builtIn",
	"screenshot.dialog",
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
	const kind = preconditionCallPaths.has(call.path)
		? "precondition"
		: toolCallPaths.has(call.path)
			? "tool"
			: expectationKind(call.path, call.root);
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
		contextLabel: contextLabelFor(call),
		presentation: presentationFor(call),
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
		.filter((part) => part !== "via" && part !== "expect")
		.map(humanize)
		.join(" → ");
	const subject = `${words.charAt(0).toUpperCase()}${words.slice(1)}`;
	if (kind === "expected-outcome")
		return `${subject}${args.length ? `: ${args.join(", ")}` : ""}.`;
	return `${subject}${args.length ? ` with ${args.join(", ")}` : ""}.`;
}

function describeActiveShowReference(value) {
	const result = /^\$[A-Za-z_$][\w$]* \(result of ([^)]+)\)$/u.exec(
		value ?? "",
	);
	if (!result) return describeShowReference(value);
	const descriptions = {
		"show.create": "the created show",
		"show.loadRevision": "the loaded revision copy",
		"show.saveAs": "the saved show copy",
		"show.via.ui.loadRevision": "the loaded revision copy",
	};
	return descriptions[result[1]] ?? value;
}

function humanize(value) {
	if (/^[A-Z]$/u.test(value)) return value;
	return value
		.replace(/([a-z0-9])([A-Z])/gu, "$1 $2")
		.replaceAll("_", " ")
		.toLowerCase();
}

function resultTypeFor(callPath) {
	if (callPath === "cue.openEditor" || callPath === "cue.reopenEditor")
		return "cueEditor";
	if (callPath === "cueEditor.openSettings") return "cueListSettings";
	if (callPath === "desktop.configure") return "desktopBuilder";
	if (callPath === "screen.create") return "screenHandle";
	return undefined;
}

function contextLabelFor(call) {
	if (call.path !== "show.use") return undefined;
	const show = call.arguments[0];
	if (!show || show.includes("<unresolved:")) return undefined;
	return { kind: "show", label: describeShowReference(show) };
}

function describeShowReference(show) {
	const enumMember = /^Show\.([A-Za-z0-9_]+)$/u.exec(show);
	return (enumMember?.[1] ?? show)
		.replace(/([a-z0-9])([A-Z])/gu, "$1 $2")
		.replaceAll("_", " ");
}

function presentationFor(call) {
	if (call.path === "desktopBuilder.addPane")
		return { omitStructuredArguments: true };
	if (call.path === "desktop.configure") {
		const label = literalLabel(call.arguments[0]);
		return label
			? { badges: [{ kind: "desktop", label }], omitArguments: [0] }
			: undefined;
	}
	if (call.path === "screen.create") {
		const label = objectStringField(call.arguments[0], "name");
		return {
			badges: label ? [{ kind: "screen", label }] : [],
			omitConfigurationFields: ["name", "display", "bounds"],
		};
	}
	return undefined;
}

function literalLabel(value) {
	if (!value) return undefined;
	try {
		const parsed = JSON.parse(value);
		return typeof parsed === "string" ? parsed : undefined;
	} catch {
		return undefined;
	}
}

function objectStringField(value, field) {
	if (field !== "name") return undefined;
	const match = /(?:^|[{,]\s*)name:\s*("(?:\\.|[^"\\])*")/u.exec(value ?? "");
	return literalLabel(match?.[1]);
}

export function catalogFamilies() {
	return {
		world: Object.keys(worldFamilies).sort(),
		returned: Object.keys(returnedFamilies).sort(),
	};
}
