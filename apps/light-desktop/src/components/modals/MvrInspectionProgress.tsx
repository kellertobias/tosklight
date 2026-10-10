import { OperationBusyOverlay } from "@tosklight/ui";
export function MvrInspectionProgress({
	operation,
	startedAt,
	file,
	onCancel,
}: {
	operation: "read" | "inspect" | "apply";
	startedAt: number;
	file: { name: string; size: number } | null;
	onCancel: () => void;
}) {
	return (
		<OperationBusyOverlay
			label="MVR operation progress"
			title={
				operation === "read"
					? "Loading selected MVR file…"
					: operation === "inspect"
						? "Inspecting MVR archive and fixture data"
						: "Applying MVR to the show"
			}
			startedAt={startedAt}
			source={
				file
					? `${file.name} · ${(file.size / 1_000_000).toFixed(2)} MB`
					: undefined
			}
			message={
				operation === "read"
					? "Reading the selected MVR file…"
					: operation === "inspect"
						? "The current show is unchanged during inspection."
						: "Adding fixtures and placements to the show…"
			}
			onCancel={operation === "inspect" ? onCancel : undefined}
			cancelLabel="Cancel inspection"
		/>
	);
}
