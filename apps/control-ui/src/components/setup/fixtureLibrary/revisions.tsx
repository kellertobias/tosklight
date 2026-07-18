import { useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { FixtureDefinition, FixtureProfile } from "../../../api/types";
import { Button } from "../../common";

interface FixtureRevisionHistoryOptions {
	selectedMode: FixtureDefinition | null;
	onEditRevision: (profile: FixtureProfile, expectedRevision: number) => void;
}

export function useFixtureRevisionHistory({
	selectedMode,
	onEditRevision,
}: FixtureRevisionHistoryOptions) {
	const server = useServer();
	const [history, setHistory] = useState<FixtureProfile[] | null>(null);
	const [error, setError] = useState("");

	const open = async () => {
		if (!selectedMode) return;
		setError("");
		try {
			setHistory(
				await server.fixtureProfileRevisions(
					selectedMode.profile_id ?? selectedMode.id,
				),
			);
		} catch (reason) {
			setHistory([]);
			setError(reason instanceof Error ? reason.message : String(reason));
		}
	};

	const deleteRevision = async (profile: FixtureProfile) => {
		if (
			!window.confirm(
				`Delete ${profile.manufacturer} ${profile.name} revision ${profile.revision}? Patched shows keep their embedded snapshot.`,
			)
		) {
			return;
		}
		await server.deleteFixtureProfile(profile.id, profile.revision);
		const remaining = await server
			.fixtureProfileRevisions(profile.id)
			.catch(() => []);
		setHistory(remaining);
		if (!remaining.length) setHistory(null);
	};

	const editRevision = (profile: FixtureProfile) => {
		if (!history) return;
		onEditRevision(
			profile,
			Math.max(...history.map((revision) => revision.revision)),
		);
		setHistory(null);
	};

	return {
		close: () => setHistory(null),
		deleteRevision,
		editRevision,
		error,
		history,
		open,
	};
}

interface FixtureRevisionHistoryProps {
	history: FixtureProfile[];
	error: string;
	onClose: () => void;
	onDelete: (profile: FixtureProfile) => Promise<void>;
	onEdit: (profile: FixtureProfile) => void;
}

export function FixtureRevisionHistory({
	history,
	error,
	onClose,
	onDelete,
	onEdit,
}: FixtureRevisionHistoryProps) {
	return (
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) =>
				event.target === event.currentTarget && onClose()
			}
		>
			<section
				className="nested-modal fixture-revision-history"
				role="dialog"
				aria-modal="true"
				aria-label="Fixture revision history"
			>
				<header>
					<h2>Fixture revision history</h2>
					<Button aria-label="Close Fixture revision history" onClick={onClose}>
						×
					</Button>
				</header>
				{error && <p role="alert">{error}</p>}
				{!history.length && !error && <p>No retained revisions.</p>}
				<div>
					{[...history]
						.sort((left, right) => right.revision - left.revision)
						.map((profile) => (
							<article key={`${profile.id}:${profile.revision}`}>
								<span>
									<b>Revision {profile.revision}</b>
									<small>
										{profile.manufacturer} {profile.name} ·{" "}
										{profile.modes.length} mode
										{profile.modes.length === 1 ? "" : "s"}
									</small>
								</span>
								<Button onClick={() => onEdit(profile)}>
									Edit as new revision
								</Button>
								<Button
									className="danger"
									onClick={() => void onDelete(profile)}
								>
									Delete revision
								</Button>
							</article>
						))}
				</div>
				<p>
					Deleting a library revision never changes fixtures already patched
					into a show because each patch embeds its own portable snapshot.
				</p>
			</section>
		</div>
	);
}
