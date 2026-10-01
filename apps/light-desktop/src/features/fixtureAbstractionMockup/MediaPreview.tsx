import { useId } from "react";
import "./MediaPreview.css";

const SOURCE_COLORS = [
	"#ff5738", "#f5c54d", "#82c251", "#43a9ce", "#6559ce", "#d153ab",
	"#d4e5ed", "#000000", "#345263", "#ffffff", "#d38952", "#62b59b",
];

const unit = (percent: number) => Math.max(0, Math.min(100, percent)) / 100;
const decode = (value: number) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
const encode = (value: number) => value <= 0.0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - 0.055;

function TestCard({ rgb, whiteBlend, intensity, source = false }: {
	rgb: [number, number, number];
	whiteBlend: number;
	intensity: number;
	source?: boolean;
}) {
	const checkerId = `media-checker-${useId().replace(/:/g, "")}`;
	const tint = rgb.map((value) => decode(unit(value)));
	const blend = unit(whiteBlend);
	return <svg viewBox="0 0 360 180" role="img" aria-label={source ? "Original media test card" : "Processed media test card"} className="fam-inline-media-card">
		<defs><pattern id={checkerId} width="16" height="16" patternUnits="userSpaceOnUse">
			<rect width="16" height="16" fill="#354250" />
			<rect width="8" height="8" fill="#687482" />
			<rect x="8" y="8" width="8" height="8" fill="#687482" />
		</pattern></defs>
		<rect width="360" height="180" fill={`url(#${checkerId})`} />
		{SOURCE_COLORS.map((color, index) => {
			const linear = [1, 3, 5].map((at) => decode(Number.parseInt(color.slice(at, at + 2), 16) / 255));
			const luminance = linear[0] * .2126 + linear[1] * .7152 + linear[2] * .0722;
			const output = linear.map((value, channel) => Math.round(255 * encode(
				((1 - blend) * value + blend * luminance) * tint[channel] * unit(intensity),
			)));
			return <rect key={color} data-testid={index === 7 ? "media-black-tile" : undefined}
				x={index % 6 * 60} y={Math.floor(index / 6) * 90} width="60" height="90"
				fill={source ? color : `rgb(${output.join(",")})`} fillOpacity={index === 10 ? .4 : 1} />;
		})}
	</svg>;
}

export function MediaPreview({ rgb, whiteBlend, intensity = 100 }: {
	rgb: [number, number, number];
	whiteBlend: number;
	intensity?: number;
}) {
	const blend = Math.round(unit(whiteBlend) * 100);
	const neutral = rgb.every((value) => value === 100);
	const treatment = blend === 0 ? "Original color" : blend === 100 ? "Grayscale" : `${blend}% grayscale`;
	return <div className="fam-inline-media">
		<div className="fam-inline-media-previews">
			<figure><figcaption>Source</figcaption><TestCard source rgb={[100, 100, 100]} whiteBlend={0} intensity={100} /></figure>
			<figure><figcaption>Output</figcaption><TestCard rgb={rgb} whiteBlend={whiteBlend} intensity={intensity} /></figure>
		</div>
		<p className="fam-inline-media-treatment">{treatment}{neutral ? "" : " · RGB tint"}</p>
	</div>;
}
