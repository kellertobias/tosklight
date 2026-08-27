import fs from "node:fs/promises";
import path from "node:path";
import { artifactPaths } from "../tools/artifact-paths.mjs";
import { expect, test } from "./bench/core/fixtures";

/**
 * The Media pane browses a CITP server by asking it what it advertises. An Internal Audio Player
 * holds no such conversation, so the desk has to hand the pane its indexed library or every slot
 * reads as empty.
 */
test("AUDIO-LIBRARY-001 @api the Internal Audio Player reports its library to the Media pane", async ({
	api,
}) => {
	const library = path.join(
		artifactPaths.tmp,
		`audio-player-library-${crypto.randomUUID()}`,
	);
	await fs.mkdir(path.join(library, "001"), { recursive: true });
	await fs.mkdir(path.join(library, "002"), { recursive: true });
	await fs.writeFile(path.join(library, "001", "001.Walk in.wav"), "pcm");
	await fs.writeFile(path.join(library, "001", "003.Intro.wav"), "pcm");
	await fs.writeFile(path.join(library, "002", "001.Encore.wav"), "pcm");
	try {
		await api.request("PUT", "/api/v2/configuration", {
			internal_audio_library_roots: { default: library },
		});
		await patchAudioPlayer(api);
		const servers = await api.request<{ fixtures: Array<Record<string, any>> }>(
			"GET",
			"/api/v2/media-servers",
		);
		const player = servers.fixtures.find(
			(server) => server.kind === "audio_player",
		);
		expect(player, "the Audio Player is patched").toBeTruthy();

		expect(player.audio.library).toEqual(
			expect.arrayContaining([
				{ folder: 1, file: 1, name: "001/001.Walk in.wav" },
				{ folder: 1, file: 3, name: "001/003.Intro.wav" },
				{ folder: 2, file: 1, name: "002/001.Encore.wav" },
			]),
		);
	} finally {
		await api.request("PUT", "/api/v2/configuration", {
			internal_audio_library_roots: {},
		});
		await fs.rm(library, { recursive: true, force: true });
	}
});

/**
 * The Audio Player is a media source like any other, so the Media pane's own playback controls
 * have to work on it: play mode to start and stop it, volume to set its level. Nothing about it
 * being audio makes those different controls.
 */
test("AUDIO-CONTROLS-001 @api the Media pane offers an Audio Player play mode and volume", async ({
	api,
}) => {
	await patchAudioPlayer(api);
	const servers = await api.request<{ fixtures: Array<Record<string, any>> }>(
		"GET",
		"/api/v2/media-servers",
	);
	const player = servers.fixtures.find(
		(server) => server.kind === "audio_player",
	);
	expect(player, "the Audio Player is patched").toBeTruthy();

	// The pane enables a control when the selected layer owns its attribute, so what the layer
	// advertises is what the operator can reach.
	const layer = player.layers?.[0];
	expect(layer, "the player offers a layer to select").toBeTruthy();
	expect(layer.attributes).toEqual(
		expect.arrayContaining(["media.play_mode", "volume"]),
	);

	// And it reaches them through the ordinary Media attributes rather than anything audio-only,
	// which is what puts it on the same encoders as any other player.
	expect(layer.attributes).toEqual(
		expect.arrayContaining(["media.folder", "media.file"]),
	);
	expect(
		layer.attributes.filter((attribute: string) =>
			attribute.startsWith("audio."),
		),
		"nothing audio-specific is exposed",
	).toEqual([]);
});

/// Patches one shipped Internal Audio Player, which has no DMX footprint and no address.
async function patchAudioPlayer(api: {
	request<T>(
		method: string,
		path: string,
		body?: unknown,
		authenticate?: boolean,
		revision?: number,
	): Promise<T>;
}): Promise<void> {
	const library = await api.request<{ profiles: Array<Record<string, any>> }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find(
		(candidate) =>
			candidate.manufacturer === "ToskLight" &&
			candidate.name === "Audio Player",
	);
	expect(profile, "the shipped Audio Player profile is loaded").toBeTruthy();
	const mode = profile?.modes?.[0];
	expect(mode, "the Audio Player has a mode").toBeTruthy();
	const patch = await api.request<{ patch_revision: number }>(
		"GET",
		"/api/v2/patch",
	);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
		request_id: crypto.randomUUID(),
		fixtures: [
			{
				fixture_id: crypto.randomUUID(),
				fixture_number: 901,
				virtual_fixture_number: null,
				name: "Playback audio",
				profile_id: profile.id,
				profile_revision: profile.revision,
				mode_id: mode.id,
				// An internal fixture owns no DMX address.
				split_patches: [{ split: 1, universe: null, address: null }],
				layer_id: "default",
				direct_control: null,
				location: { x: 0, y: 0, z: 0 },
				rotation: { x: 0, y: 0, z: 0 },
				multipatch: [],
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
				highlight_overrides: [],
			},
		],
		remove_fixture_ids: [],
		},
		true,
		patch.patch_revision,
	);
}
