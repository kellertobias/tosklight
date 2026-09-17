import {
	Button,
	FormLayout,
	NumberField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useCallback, useEffect, useState } from "react";
import type { PsnConfiguration, PsnEdit } from "../../api/client/psn";
import { usePsn } from "../../features/psn/PsnContext";

/** The receive switch stays on the Tracking page: it is what an operator reaches for there. */
export function PsnReceiveSwitch({
	configuration,
	busy,
	onEdit,
}: {
	configuration: PsnConfiguration;
	busy: boolean;
	onEdit: (edit: PsnEdit) => void;
}) {
	return (
		<FormLayout columns={1}>
			<SwitchField
				label="Receive PosiStageNet"
				offLabel="Off"
				onLabel="Listening"
				checked={configuration.enabled}
				disabled={busy}
				onChange={(event) => onEdit({ enabled: event.target.checked })}
			/>
		</FormLayout>
	);
}

type SourceDraft = { group: string; port: string; staleAfter: string };
type SourceErrors = Partial<Record<keyof SourceDraft, string>>;

/** The same limits the desk enforces, said before the request so the operator can fix them. */
export function validateTrackingSource(draft: SourceDraft): SourceErrors {
	const errors: SourceErrors = {};
	const octets = draft.group.trim().split(".");
	const valid =
		octets.length === 4 &&
		octets.every((octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255);
	if (!valid)
		errors.group = "Enter an IPv4 multicast group, for example 236.10.10.10.";
	else if (Number(octets[0]) < 224 || Number(octets[0]) > 239)
		errors.group =
			"PosiStageNet transmits to a multicast group: use 224.0.0.0 to 239.255.255.255.";
	const port = Number(draft.port);
	if (!Number.isInteger(port) || port < 1 || port > 65_535)
		errors.port = "Enter a port from 1 to 65535.";
	const stale = Number(draft.staleAfter);
	if (!Number.isInteger(stale) || stale < 50 || stale > 60_000)
		errors.staleAfter = "Enter a whole number from 50 to 60000 milliseconds.";
	return errors;
}

function draftOf(configuration: PsnConfiguration): SourceDraft {
	return {
		group: configuration.group,
		port: String(configuration.port),
		staleAfter: String(configuration.staleAfterMillis),
	};
}

/**
 * Tracking Settings: where the desk listens and when a silent tracker counts as stale.
 *
 * Values are applied together with one explicit action, so a half-typed multicast address is never
 * sent and every refusal — the surface's or the desk's — is shown beside the field it concerns.
 */
export function TrackingSettingsForm() {
	const psn = usePsn();
	const [stored, setStored] = useState<PsnConfiguration | null>(null);
	const [draft, setDraft] = useState<SourceDraft | null>(null);
	const [errors, setErrors] = useState<SourceErrors>({});
	const [failure, setFailure] = useState<string | null>(null);
	const [saved, setSaved] = useState(false);
	const [busy, setBusy] = useState(false);

	const load = useCallback(async () => {
		if (!psn) return;
		try {
			const snapshot = await psn.snapshot();
			setStored(snapshot.configuration);
			setDraft(draftOf(snapshot.configuration));
			setFailure(null);
		} catch (cause) {
			setFailure(
				`The tracking settings could not be read: ${cause instanceof Error ? cause.message : String(cause)}. Check the desk connection and reopen Settings.`,
			);
		}
	}, [psn]);

	useEffect(() => {
		void load();
	}, [load]);

	if (!psn)
		return <p role="alert">Tracking is not available on this surface.</p>;
	if (!stored || !draft)
		return (
			<section className="tracking-settings">
				<p role={failure ? "alert" : undefined}>
					{failure ?? "Reading the tracking settings…"}
				</p>
			</section>
		);

	const change = (field: keyof SourceDraft, value: string) => {
		setDraft({ ...draft, [field]: value });
		setErrors((current) => ({ ...current, [field]: undefined }));
		setSaved(false);
	};
	const edit: PsnEdit = {};
	if (draft.group.trim() !== stored.group) edit.group = draft.group.trim();
	if (Number(draft.port) !== stored.port) edit.port = Number(draft.port);
	if (Number(draft.staleAfter) !== stored.staleAfterMillis)
		edit.staleAfterMillis = Number(draft.staleAfter);
	const dirty = Object.keys(edit).length > 0;

	const apply = async () => {
		const found = validateTrackingSource(draft);
		setErrors(found);
		setSaved(false);
		if (Object.keys(found).length) return;
		setBusy(true);
		setFailure(null);
		try {
			await psn.update(edit);
			await load();
			setSaved(true);
		} catch (cause) {
			setFailure(
				`The desk refused the tracking settings: ${cause instanceof Error ? cause.message : String(cause)}. Correct the value and apply again.`,
			);
		} finally {
			setBusy(false);
		}
	};

	return (
		<section className="tracking-settings">
			<h3>PosiStageNet source</h3>
			<FormLayout columns={1} labelPlacement="top">
				<TextField
					label="Multicast group"
					value={draft.group}
					error={errors.group}
					disabled={busy}
					onChange={(event) => change("group", event.target.value)}
				/>
				<NumberField
					label="Port"
					min="1"
					max="65535"
					value={draft.port}
					error={errors.port}
					disabled={busy}
					onChange={(event) => change("port", event.target.value)}
				/>
				<NumberField
					label="Stale after (ms)"
					description="How long without a packet before a tracker is called stale. A stale tracker still holds its last position."
					min="50"
					max="60000"
					value={draft.staleAfter}
					error={errors.staleAfter}
					disabled={busy}
					onChange={(event) => change("staleAfter", event.target.value)}
				/>
			</FormLayout>
			{failure && <p role="alert">{failure}</p>}
			<div className="tracking-settings-actions">
				{saved && !dirty && <span role="status">Tracking settings saved.</span>}
				<Button disabled={busy || !dirty} onClick={() => void apply()}>
					{busy ? "Applying…" : "Apply tracking settings"}
				</Button>
				<Button
					disabled={busy || !dirty}
					onClick={() => {
						setDraft(draftOf(stored));
						setErrors({});
					}}
				>
					Revert
				</Button>
			</div>
		</section>
	);
}
