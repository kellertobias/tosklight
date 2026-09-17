// Choosing the folder that holds this Media Server's configuration and media library.
//
// The folders listed are on the Media Server computer, not the browser's, so the picker browses
// through the server. Choosing a folder is validated there; a refusal is shown here and changes
// nothing, and an accepted folder restarts the server to load it.

import { Button } from "@tosklight/ui/controls";
import { ModalFrame } from "@tosklight/ui/modals";
import { useCallback, useEffect, useState } from "react";
import { api } from "../../shared/api/client";
import { requestId } from "../../shared/api/editing";
import type {
	DataFolderChangeView,
	DataFolderListingView,
} from "../../shared/api/generated/media-wire";

function failureText(error: unknown, fallback: string): string {
	return error instanceof Error && error.message ? error.message : fallback;
}

export function DataFolderPicker({
	onClose,
	onChanged,
}: {
	onClose: () => void;
	onChanged: (change: DataFolderChangeView) => void;
}) {
	const [listing, setListing] = useState<DataFolderListingView>();
	const [loading, setLoading] = useState(true);
	const [choosing, setChoosing] = useState(false);
	const [failure, setFailure] = useState<string>();

	const open = useCallback(async (directory?: string) => {
		setLoading(true);
		setFailure(undefined);
		try {
			setListing(await api.dataFolders(directory));
		} catch (error) {
			setFailure(failureText(error, "That folder could not be opened."));
		} finally {
			setLoading(false);
		}
	}, []);
	useEffect(() => void open(), [open]);

	const choose = async () => {
		if (!listing) return;
		setChoosing(true);
		setFailure(undefined);
		try {
			const change = await api.updateDataFolder({
				requestId: requestId(),
				directory: listing.directory,
			});
			onChanged(change);
		} catch (error) {
			const reason = failureText(error, "That folder cannot be used.");
			setFailure(
				`${reason}${/[.!?]$/u.test(reason) ? "" : "."} The current folder is still in use.`,
			);
		} finally {
			setChoosing(false);
		}
	};

	return (
		<ModalFrame
			id="media-data-folder-picker"
			ariaLabel="Choose media and configuration folder"
			title="Choose folder"
			details="Folders on the Media Server computer"
			closeLabel="Cancel folder change"
			closeDisabled={choosing}
			dialogClassName="media-folder-picker"
			onClose={onClose}
		>
			<div className="media-folder-picker-body" aria-busy={loading || choosing}>
				<code className="media-data-directory" aria-label="Open folder">
					{listing?.directory ?? "…"}
				</code>
				{listing?.hasConfiguration && (
					<p className="media-state is-notice">
						This folder already holds a Media Server configuration. Choosing it
						loads that configuration and its media library.
					</p>
				)}
				<ul className="media-folder-picker-list" aria-label="Folders">
					{listing?.parent && (
						<li>
							<Button
								disabled={loading || choosing}
								onClick={() => void open(listing.parent ?? undefined)}
							>
								↑ Up one folder
							</Button>
						</li>
					)}
					{listing?.folders.map((folder) => (
						<li key={folder.directory}>
							<Button
								disabled={loading || choosing}
								onClick={() => void open(folder.directory)}
							>
								{folder.name}{" "}
								{folder.hasConfiguration && (
									<span className="media-folder-picker-badge">
										Configuration
									</span>
								)}
							</Button>
						</li>
					))}
					{listing && listing.folders.length === 0 && (
						<li className="media-folder-picker-empty">No subfolders</li>
					)}
				</ul>
				{failure && (
					<p className="media-state is-error" role="alert">
						{failure}
					</p>
				)}
				<p>
					Pixel uses the chosen folder for its configuration and its media
					library, and restarts to load it. Media is not copied from the current
					folder.
				</p>
				<div className="media-settings-actions">
					<Button
						variant="primary"
						disabled={!listing || loading || choosing}
						onClick={() => void choose()}
					>
						{choosing ? "Checking folder…" : "Use this folder"}
					</Button>
					<Button disabled={choosing} onClick={onClose}>
						Cancel
					</Button>
				</div>
			</div>
		</ModalFrame>
	);
}
