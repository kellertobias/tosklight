/**
 * The lighting designer's company logo as a show keeps it: one small JPEG with its pixel size,
 * stored as JSON beside the rest of the show's paperwork.
 */

export interface CompanyLogo {
	mediaType: "image/jpeg";
	width: number;
	height: number;
	/** The JPEG bytes, base64-encoded. */
	data: string;
}

/** The logo a show stores, or null when it has none or what it stores is not a logo. */
export function parseCompanyLogo(stored: string | null | undefined): CompanyLogo | null {
	if (!stored?.trim()) return null;
	try {
		const logo = JSON.parse(stored) as Partial<CompanyLogo>;
		return logo.mediaType === "image/jpeg" &&
			typeof logo.data === "string" &&
			Number(logo.width) > 0 &&
			Number(logo.height) > 0
			? (logo as CompanyLogo)
			: null;
	} catch {
		return null;
	}
}

/** The logo as an image source a page can show. */
export function companyLogoUrl(logo: CompanyLogo): string {
	return `data:${logo.mediaType};base64,${logo.data}`;
}

/** The logo's JPEG bytes as hexadecimal text, which a PDF stream can carry unescaped. */
export function companyLogoHex(logo: CompanyLogo): string {
	const binary = atob(logo.data);
	let hex = "";
	for (let index = 0; index < binary.length; index++)
		hex += binary.charCodeAt(index).toString(16).padStart(2, "0");
	return hex;
}
