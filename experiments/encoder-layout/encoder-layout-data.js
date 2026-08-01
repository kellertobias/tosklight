/*
 * This is the only file that needs changing when an encoder is renamed, moved,
 * added, removed, or paired with another encoder.
 *
 * `blocks` are packed into pages without splitting. Put a related pair in one
 * block to keep it together. `orders[4|5|6]` may override block order for one
 * hardware width when the most useful grouping genuinely differs.
 */
window.ENCODER_LAYOUT = {
	defaultGroup: "intensity",
	defaultWidth: 6,
	groups: [
		{
			id: "intensity",
			label: "Intensity",
			kind: "Fixture attributes",
			description: "Output amount and intensity-adjacent mechanisms.",
			blocks: [
				[encoder("intensity", "Intensity", "Overall light output for the selected fixture or head.")],
				[encoder("shutter-strobe", "Shutter / Strobe", "Open, closed, pulse, random, and continuous strobe functions on one mechanism.")],
				[encoder("mask-opacity", "Mask Opacity", "Strength of a media layer's mask; grouped here because it controls how much of the layer remains visible.")],
				[encoder("volume", "Volume", "Level for fixtures whose primary output is audio.")],
			],
		},
		{
			id: "color",
			label: "Color",
			kind: "Fixture attributes",
			description: "Canonical emitter controls, white point, and numbered color wheels.",
			blocks: [
				[encoder("red", "Red", "Canonical additive red; physical Cyan filtration maps inversely here.")],
				[encoder("green", "Green", "Canonical additive green; physical Magenta filtration maps inversely here.")],
				[encoder("blue", "Blue", "Canonical additive blue; physical Yellow filtration maps inversely here.")],
				[encoder("white", "White", "White emitter level; a physical cold-white emitter maps here.")],
				[encoder("amber", "Amber", "Amber emitter level; a physical warm-white emitter maps here.")],
				[encoder("uv", "UV", "Explicit ultraviolet or violet emitter level.")],
				[encoder("lime", "Lime", "Additional lime emitter level when independently controllable.")],
				[encoder("indigo", "Indigo", "Additional indigo emitter level when independently controllable.")],
				[encoder("mint", "Mint", "Additional mint emitter level when independently controllable.")],
				[encoder("temperature", "Temperature", "Correlated color temperature for tunable-white fixtures.")],
				[encoder("color-wheel-1", "Color Wheel 1", "Selection, indexing, and rotation functions for the first wheel.")],
				[encoder("color-wheel-2", "Color Wheel 2", "Selection, indexing, and rotation functions for the second wheel.")],
			],
		},
		{
			id: "position",
			label: "Position",
			kind: "Fixture attributes",
			description: "Absolute or continuous axes, shared movement timing, and element rotation.",
			blocks: [
				[encoder("pan", "Pan", "Absolute Pan with continuous-spin functions available on the same axis.")],
				[encoder("tilt", "Tilt", "Absolute Tilt with continuous-spin functions available on the same axis.")],
				[encoder("position-movement", "Position Movement", "Shared Pan/Tilt speed or time behavior; final canonical unit remains under review.")],
				[encoder("position-rotation", "Rotation", "Rotation of a head, element, or compatible media layer.")],
			],
		},
		{
			id: "beam",
			label: "Beam",
			kind: "Fixture attributes",
			description: "Optical effect mechanisms; Prism and Animation use push-turn for their rotation or indexing value.",
			blocks: [
				pair("gobo-1", "Gobo 1", "Gobo 1 Rotation"),
				pair("gobo-2", "Gobo 2", "Gobo 2 Rotation"),
				[encoder("prism-1", "Prism 1", "Turn selects or inserts Prism 1; push-turn controls its independent indexing or rotation channel.")],
				[encoder("prism-2", "Prism 2", "Turn selects or inserts Prism 2; push-turn controls its independent indexing or rotation channel.")],
				[encoder("animation-1", "Animation", "Turn selects or positions the animation wheel; push-turn controls its independent rotation channel.")],
				[
					encoder("media-effect-1", "Media Effect 1", "First media or projection effect slot; grouped with Beam because it changes the rendered image or beam texture."),
					encoder("media-effect-2", "Media Effect 2", "Second media or projection effect slot; grouped with Beam because it changes the rendered image or beam texture."),
					encoder("media-effect-3", "Media Effect 3", "Third media or projection effect slot; grouped with Beam because it changes the rendered image or beam texture."),
					encoder("media-effect-4", "Media Effect 4", "Fourth media or projection effect slot; grouped with Beam because it changes the rendered image or beam texture."),
				],
			],
		},
		{
			id: "shapers",
			label: "Shapers",
			kind: "Fixture attributes",
			description: "Four framing blades, their shared module rotation, and iris.",
			blocks: [
				[encoder("iris", "Iris", "Adjusts the circular beam aperture.")],
				pair("blade-1", "Blade 1 Position", "Blade 1 Angle"),
				pair("blade-2", "Blade 2 Position", "Blade 2 Angle"),
				[encoder("shaper-rotation", "Shaper Rotation", "Rotates the complete framing or barn-door module.")],
				pair("blade-3", "Blade 3 Position", "Blade 3 Angle"),
				pair("blade-4", "Blade 4 Position", "Blade 4 Angle"),
			],
			orders: {
				4: ["blade-1", "blade-2", "blade-3", "blade-4", "iris", "shaper-rotation"],
			},
		},
		{
			id: "focus",
			label: "Focus",
			kind: "Fixture attributes",
			description: "Optical focus, beam size, and the canonical beam-softness control.",
			blocks: [
				[encoder("focus", "Focus", "Moves the focal plane for a sharp or soft projection.")],
				[encoder("zoom", "Zoom", "Changes the beam angle or projected image size.")],
				[encoder("softness", "Softness", "Canonical softening control mapped by the fixture profile to Frost or Beam Edge. Additional independently controllable frost or edge mechanisms require custom attributes.")],
			],
		},
		{
			id: "control",
			label: "Control",
			kind: "Fixture attributes",
			description: "Fixture functions and media playback behavior that determine how content runs.",
			blocks: [
				[encoder("control", "Control", "Opens fixture-authored indexed and typed control functions.")],
				[encoder("play-mode", "Play Mode", "Loop, once, bounce, hold, or another server-authored playback mode.")],
				[encoder("playback-speed", "Playback Speed", "Continuous media playback speed.")],
				[encoder("playback-bpm", "Playback BPM", "Beat-oriented playback rate when the server supports it.")],
			],
		},
		{
			id: "media",
			label: "Media",
			kind: "Fixture attributes",
			description: "Coherent media and mask source selection.",
			blocks: [
				pair("media-source", "Folder", "File"),
				pair("mask-source", "Mask Folder", "Mask File"),
				[encoder("mask-invert", "Mask Invert", "Switches the selected mask between normal and inverted behavior.")],
			],
		},
		{
			id: "dynamic-running",
			label: "Running Dynamic",
			kind: "Application controls",
			description: "Live overrides for one selected running Dynamic; instance and lane selection remain in navigation.",
			blocks: [
				[encoder("instance-size", "Instance Size", "Amount of this running Dynamic: 0% removes its excursion and 100% uses the authored result.")],
				[encoder("instance-speed", "Instance Speed", "Overall speed multiplier for every lane in this running copy.")],
				[encoder("instance-phase", "Instance Phase", "Moves this running copy forward or backward through its cycle.")],
				[encoder("dynamic-off", "Dynamic Off", "Press to stop only this exact running instance.")],
			],
		},
		{
			id: "dynamic-curve",
			label: "Dynamic Curve",
			kind: "Application controls",
			description: "Structural configuration of the selected scalar Dynamic lane.",
			blocks: [
				[encoder("curve-shape", "Curve Shape", "Turn selects function; push-turn selects Keyframes, Max/min, or Middle/amplitude.")],
				[encoder("curve-high", "Top / Middle", "Upper bound or midpoint, depending on the selected curve method.")],
				[encoder("curve-low", "Bottom / Amplitude", "Lower bound or amplitude, depending on the selected curve method.")],
				[encoder("curve-width", "Curve Width", "Compresses a non-PWM curve into the middle of its cycle.")],
				[encoder("lane-speed", "Lane Speed", "Saved speed multiplier for only the selected lane.")],
				[encoder("keyframe-selector", "Keyframe", "Selects or reports the active keyframe when the lane uses keyframes.")],
			],
		},
		{
			id: "dynamic-pwm",
			label: "Dynamic PWM",
			kind: "Application controls",
			description: "Timing envelope for a PWM lane; Off is currently complementary to On.",
			blocks: [
				[
					encoder("pwm-attack", "Attack", "Rise from Bottom to Top."),
					encoder("pwm-on", "On", "High-side portion of the cycle, including Attack in the current model."),
					encoder("pwm-decay", "Decay", "Fall from Top to Bottom."),
					encoder("pwm-off", "Off", "Remaining low-side portion; currently derived as 100% minus On."),
				],
				pair("pwm-interpolation", "Attack Interpolation", "Decay Interpolation"),
			],
		},
		{
			id: "timecode-timeline",
			label: "Timecode Timeline",
			kind: "Application controls · prototype",
			description: "Exploratory timeline navigation and loop-range controls; no production runtime is implied.",
			blocks: [
				[encoder("timeline-zoom", "Horizontal Zoom", "Scales the visible time range.")],
				[encoder("playhead", "Playhead", "Moves through the timeline at frame precision.")],
				[encoder("loop-range", "Loop Start / End", "Turn edits Start; push-turn edits End while preserving a valid range.")],
				[encoder("vertical-zoom", "Vertical Zoom", "Scales lane height in the editor.")],
				[encoder("beat-alignment", "Beat Alignment", "Preserve Speed Group phase or restart it at Beat 1 at the selected point.")],
			],
		},
	],
};

function encoder(id, label, description) {
	return { id, label, description };
}

function pair(id, first, second) {
	return [
		encoder(`${id}-primary`, first, `${first} for this physical mechanism.`),
		encoder(`${id}-secondary`, second, `${second} for this physical mechanism.`),
	];
}
