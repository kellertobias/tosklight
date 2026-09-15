import { describe, expect, it } from "vitest";
import { type CadPrintDocumentInfo, buildCadPdf } from "./print";
import type { CadPrintPage, CadSceneSnapshot } from "./types";

const scene: CadSceneSnapshot = {
	showId: "show",
	sceneRevision: 1,
	selectionRevision: 1,
	selectedIds: [],
	attachments: [],
	drawings: [],
	entities: [],
};

// Cast rather than annotated, so the fixture type-checks whichever optional page switches this build has.
const plan = {
	id: "plan",
	tileId: "tile",
	name: "Plan",
	view: "top_down",
	rotationQuarterTurns: 0,
	centreMillimetres: [0, 0],
	widthMillimetres: 5000,
	included: true,
	orientation: "landscape",
	showFixtureIds: true,
	showDmxAddresses: true,
	showMountingHardware: true,
} as CadPrintPage;

const fixtureList: CadPrintPage = { ...plan, id: "list", kind: "fixture_list", name: "Fixtures" };

// Four bytes of a JPEG header are enough for the PDF to carry; no page decodes it here.
const logo = JSON.stringify({ mediaType: "image/jpeg", width: 200, height: 100, data: "/9j/4AAQ" });

function info(companyLogo = ""): CadPrintDocumentInfo {
	return {
		showName: "Summer Tour",
		lightingDesigner: "Tobias Keller",
		showVersion: "1",
		venue: "Grand Hall",
		contactEmail: "",
		contactPhone: "",
		project: "",
		showDate: "",
		companyLogo,
		lastSavedAt: 0,
		fixtureCount: 0,
		universeCount: 0,
	};
}

const pdf = (companyLogo?: string) =>
	new TextDecoder().decode(buildCadPdf(scene, [plan, fixtureList], info(companyLogo)));

describe("the company logo on printed pages", () => {
	it("prints the logo in place of the ToskLight mark on plan and fixture list pages", () => {
		const document = pdf(logo);
		expect(document).toContain(
			"/Subtype /Image /Width 200 /Height 100 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter [/ASCIIHexDecode /DCTDecode]",
		);
		// The base64 JPEG header /9j/4AAQ is FF D8 FF E0 00 10 once decoded.
		expect(document).toContain("ffd8ffe00010>");
		expect(document.match(/\/CompanyLogo Do/g)).toHaveLength(2);
		expect(document.match(/\/XObject << \/CompanyLogo \d+ 0 R >>/g)).toHaveLength(2);
	});

	it("keeps the ToskLight mark and embeds no image when the show has no logo", () => {
		const document = pdf();
		expect(document).not.toContain("/Subtype /Image");
		expect(document).not.toContain("/CompanyLogo");
	});
});
