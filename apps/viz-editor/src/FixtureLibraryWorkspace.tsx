import { useCallback, useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { Button, SearchBar } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import {
	FixtureProfileEditor,
	type FixtureProfileEditorPorts,
	type ProfileAssetPickerProps,
} from "@tosklight/patch/library";
import { blankFixtureProfile, cloneProfile } from "@tosklight/patch";
import type { AttributeDescriptor, FixtureProfile } from "@tosklight/patch";
import { FixtureLibraryBrowser } from "./fixtureLibrary/FixtureLibraryBrowser";
import { documentSession } from "./document/session";
import { beginWindowDrag } from "./WindowChrome";

/**
 * The fixture library belongs to this machine, not to the open document, so it is reachable with
 * no show open: an operator plans a rig by first describing the lanterns it is made of.
 */

/** The Architect reads the operator's own filesystem; there are no configured file roots here. */
function LocalAssetPicker({
	label,
	allowedExtensions,
	onFiles,
}: ProfileAssetPickerProps) {
	const accept = (allowedExtensions ?? [])
		.map((extension) => `.${extension.replace(/^\./, "")}`)
		.join(",");
	return (
		<label className="viz-fixture-asset-picker">
			<span className="ui-button">{label}</span>
			<input
				type="file"
				aria-label={label}
				accept={accept || undefined}
				onChange={(event) => {
					const files = Array.from(event.target.files ?? []);
					event.target.value = "";
					if (files.length) void onFiles(files);
				}}
			/>
		</label>
	);
}

/** Releases the GPU resources a preview scene holds. The Architect has no Stage renderer to ask. */
function disposeScene(scene: THREE.Object3D) {
	scene.traverse((node) => {
		const mesh = node as THREE.Mesh;
		mesh.geometry?.dispose?.();
		const material = mesh.material;
		if (Array.isArray(material))
			for (const entry of material) entry.dispose?.();
		else material?.dispose?.();
	});
}

const ports: FixtureProfileEditorPorts = {
	// No Stage renderer here: the geometry tab edits the graph and says why it cannot draw it.
	disposeScene,
	AssetPicker: LocalAssetPicker,
};

export function FixtureLibraryWorkspace({
	profiles,
	onReloadProfiles,
	onError,
}: {
	profiles: readonly FixtureProfile[];
	onReloadProfiles: () => void;
	onError: (reason: unknown) => void;
}) {
	const [registry, setRegistry] = useState<AttributeDescriptor[]>([]);
	const [query, setQuery] = useState("");
	const [manufacturer, setManufacturer] = useState<string | null>(null);
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [draft, setDraft] = useState<{
		profile: FixtureProfile;
		expectedRevision: number;
	} | null>(null);

	useEffect(() => {
		documentSession.attributeRegistry().then(setRegistry).catch(onError);
	}, [onError]);

	const manufacturers = useMemo(
		() =>
			[...new Set(profiles.map((profile) => profile.manufacturer))]
				.filter(Boolean)
				.sort(),
		[profiles],
	);

	/** The library keeps every revision; the operator browses and edits the current one. */
	const current = useMemo(() => {
		const latest = new Map<string, FixtureProfile>();
		for (const profile of profiles) {
			const held = latest.get(profile.id);
			if (!held || held.revision < profile.revision)
				latest.set(profile.id, profile);
		}
		return [...latest.values()];
	}, [profiles]);

	/** Search narrows what the columns are built from, so it filters both of them at once. */
	const shown = useMemo(() => {
		const needle = query.trim().toLowerCase();
		if (!needle) return current;
		return current.filter((profile) =>
			`${profile.manufacturer} ${profile.name} ${profile.short_name ?? ""}`
				.toLowerCase()
				.includes(needle),
		);
	}, [current, query]);

	/** A fixture stays chosen only while it is still under the chosen manufacturer and search. */
	const selected = useMemo(() => {
		const held = shown.find((profile) => profile.id === selectedId);
		return held?.manufacturer === manufacturer ? held : null;
	}, [shown, selectedId, manufacturer]);

	const save = useCallback(
		async (profile: FixtureProfile, expectedRevision: number) => {
			const saved = await documentSession.saveFixtureProfile(
				profile,
				expectedRevision,
			);
			onReloadProfiles();
			return saved;
		},
		[onReloadProfiles],
	);

	return (
		<section className="viz-fixture-library-workspace">
			<WindowHeader
				title="Fixtures"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				toolbar={
					<Button
						onClick={() =>
							setDraft({ profile: blankFixtureProfile(), expectedRevision: 0 })
						}
					>
						Create fixture
					</Button>
				}
			/>
			<SearchBar
				value={query}
				ariaLabel="Search fixtures"
				placeholder="Search manufacturer or model"
				onChange={setQuery}
			/>
			{current.length === 0 ? (
				<p className="empty-editor-message" role="status">
					This machine's fixture library is empty. Create a fixture to describe
					the lanterns this rig is made of.
				</p>
			) : (
				<FixtureLibraryBrowser
					profiles={shown}
					manufacturer={manufacturer}
					selected={selected}
					onManufacturer={(name) => {
						setManufacturer(name);
						setSelectedId(null);
					}}
					onSelect={(profile) => setSelectedId(profile.id)}
					onEdit={(profile) =>
						setDraft({
							profile: cloneProfile(profile),
							expectedRevision: profile.revision,
						})
					}
				/>
			)}
			{draft ? (
				<FixtureProfileEditor
					key={`${draft.profile.id}-${draft.expectedRevision}`}
					initialProfile={draft.profile}
					expectedRevision={draft.expectedRevision}
					manufacturers={manufacturers}
					attributeRegistry={registry}
					ports={ports}
					onSave={save}
					onClose={() => setDraft(null)}
				/>
			) : null}
		</section>
	);
}
