import { FormLayout } from "@tosklight/ui";
import { HorizontalFaderField } from "@tosklight/ui/faders";
import { useEffect, useRef, useState } from "react";
import type { ControlDesk } from "../../api/types";
import type { HardwareLightingPatch } from "../../api/types/desk";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";
import { useScreens } from "../../features/screens/ScreensContext";

const controls = [
	{ field: "hardware_led_brightness", label: "LED brightness (%)" },
	{ field: "hardware_gooseneck_brightness", label: "Gooseneck brightness (%)" },
	{
		field: "hardware_gooseneck_color",
		label: "Gooseneck color (white %)",
		description: "0 is blue, 100 is white; values between mix blue and white.",
	},
] as const;
type LightingField = (typeof controls)[number]["field"];
type LightingDraft = Record<LightingField, number>;

function values(desk: ControlDesk): LightingDraft {
	return {
		hardware_led_brightness: desk.hardware_led_brightness ?? 100,
		hardware_gooseneck_brightness: desk.hardware_gooseneck_brightness ?? 100,
		hardware_gooseneck_color: desk.hardware_gooseneck_color ?? 100,
	};
}

export function HardwareLightingSettings() {
	const connected = useHardwareConnected();
	const { session, updateControlDesk } = useScreens();
	if (!connected || !session?.desk) return null;
	return <HardwareLightingFields key={`${session.session_id}:${session.desk.id}`} desk={session.desk} save={(patch) => updateControlDesk(session.desk, { throwOnError: true, hardwareLighting: patch })} />;
}

function HardwareLightingFields({
	desk,
	save,
}: {
	desk: ControlDesk;
	save: (patch: HardwareLightingPatch) => Promise<void>;
}) {
	const [draft, setDraft] = useState(() => values(desk));
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const pending = useRef<HardwareLightingPatch>({});
	const optimistic = useRef<HardwareLightingPatch>({});
	const failures = useRef(new Map<LightingField, string>());
	const running = useRef(false);
	const active = useRef(true);
	const saveRef = useRef(save);
	saveRef.current = save;
	useEffect(() => {
		active.current = true;
		return () => {
			active.current = false;
			pending.current = {};
		};
	}, []);
	useEffect(() => {
		const snapshot = values(desk);
		// Older save responses must not move the fader away from the latest drag position.
		for (const { field } of controls) {
			if (optimistic.current[field] === snapshot[field]) delete optimistic.current[field];
		}
		setDraft({ ...snapshot, ...optimistic.current });
	}, [desk.id, desk.hardware_led_brightness, desk.hardware_gooseneck_brightness, desk.hardware_gooseneck_color]);
	const flush = async () => {
		if (running.current || !active.current) return;
		running.current = true;
		setSaving(true);
		while (active.current && Object.keys(pending.current).length) {
			const patch = pending.current;
			pending.current = {};
			try {
				await saveRef.current(patch);
				for (const { field } of controls) {
					if (patch[field] !== undefined) failures.current.delete(field);
				}
			} catch (reason) {
				if (!active.current) return;
				for (const { field } of controls) {
					if (patch[field] !== undefined) failures.current.set(field, reason instanceof Error ? reason.message : String(reason));
				}
			}
			if (active.current) setError([...new Set(failures.current.values())].join(" ") || null);
		}
		running.current = false;
		if (active.current) setSaving(false);
	};
	const change = (field: LightingField, value: number) => {
		if (!Number.isInteger(value) || value < 0 || value > 100) return;
		optimistic.current[field] = value;
		pending.current[field] = value;
		setDraft((current) => ({ ...current, [field]: value }));
		void flush();
	};
	return (
		<section className="screen-settings-card" aria-label="Hardware lighting">
			<header><div><h3>Hardware lighting</h3><p>Changes apply as you move the faders.</p></div></header>
			<FormLayout columns={3} minColumnWidth={180}>
				{controls.map((control) => (
					<HorizontalFaderField
						key={control.field}
						label={control.label}
						description={"description" in control ? control.description : "0–100%"}
						minimum={0} maximum={100} step={1}
						value={draft[control.field]}
						onChange={(value) => change(control.field, value)}
					/>
				))}
			</FormLayout>
			{error && <p role="alert">{error}</p>}
			<p role="status">{saving ? "Saving hardware lighting…" : error ? "Move a fader to retry its value." : "Hardware lighting applies automatically."}</p>
		</section>
	);
}
