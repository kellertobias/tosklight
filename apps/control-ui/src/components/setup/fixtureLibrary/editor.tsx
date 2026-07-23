import { useState } from "react";
import { useAttributeRegistry } from "../../../features/deskSnapshot/DeskSnapshotState";
import { useFixtureLibrary } from "../../../features/fixtureLibrary/FixtureLibraryContext";
import type { FixtureDefinition, FixtureProfile } from "../../../api/types";
import { FixtureProfileEditor } from "../FixtureProfileEditor";
import {
	blankFixtureProfile,
	fixtureProfileFromDefinition,
} from "../fixtureProfileModel";

export interface FixtureLibraryEditorState {
	draft: FixtureProfile;
	expectedRevision: number;
}

export function useFixtureLibraryEditor(fixtureProfiles: FixtureProfile[]) {
	const [editor, setEditor] = useState<FixtureLibraryEditorState | null>(null);

	const openCreate = () =>
		setEditor({ draft: blankFixtureProfile(), expectedRevision: 0 });

	const openSelected = (mode: FixtureDefinition) => {
		const draft = fixtureProfileFromDefinition(mode);
		setEditor({
			draft,
			expectedRevision: Math.max(
				draft.revision,
				...fixtureProfiles
					.filter((profile) => profile.id === draft.id)
					.map((profile) => profile.revision),
			),
		});
	};

	const openRevision = (profile: FixtureProfile, expectedRevision: number) => {
		setEditor({ draft: structuredClone(profile), expectedRevision });
	};

	return {
		close: () => setEditor(null),
		editor,
		openCreate,
		openRevision,
		openSelected,
	};
}

interface FixtureLibraryEditorProps {
	editor: FixtureLibraryEditorState;
	manufacturers: string[];
	onClose: () => void;
}

export function FixtureLibraryEditor({
	editor,
	manufacturers,
	onClose,
}: FixtureLibraryEditorProps) {
	const library = useFixtureLibrary();
const attributeRegistry = useAttributeRegistry();
	return (
		<FixtureProfileEditor
			initialProfile={editor.draft}
			expectedRevision={editor.expectedRevision}
			manufacturers={manufacturers}
			attributeRegistry={attributeRegistry ?? []}
			onSave={
				library?.saveFixtureProfile ??
				(async () => {
					throw new Error("The fixture library is unavailable");
				})
			}
			onClose={onClose}
		/>
	);
}
