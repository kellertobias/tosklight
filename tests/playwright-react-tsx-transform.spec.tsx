import { expect, test } from "@playwright/test";

function TransformProbe({ label }: { label: string }) {
	return <span data-transform-probe>{label}</span>;
}

test("Playwright transforms repository React TSX with the React runtime", () => {
	const probe = <TransformProbe label="ready" />;
	expect(probe.type).toBe(TransformProbe);
	expect(probe.props).toEqual({ label: "ready" });
});
