import { HorizontalFader } from "@tosklight/ui";

export function TimelineTools(props: {
	zoom: number;
	setZoom(value: number): void;
	maximumZoom: number;
}) {
	const { zoom, setZoom } = props;
	return (
		<div className="timecode-timeline-tools">
			<HorizontalFader
				className="timecode-timeline-zoom"
				label="Timeline zoom"
				minimum={1}
				maximum={props.maximumZoom}
				step={0.25}
				value={zoom}
				display={`${zoom}×`}
				onChange={setZoom}
			/>
		</div>
	);
}
