export interface PlaybackArtworkModel {
	icon?: string;
	image?: string;
	imageSource?: "configured" | "cue-preview";
	transparent?: boolean;
	notice?: string;
}

/** Decorative artwork never intercepts a Playback control gesture. */
export function PlaybackArtwork({
	artwork,
}: {
	artwork?: PlaybackArtworkModel;
}) {
	if (!artwork) return null;
	return (
		<>
			<div className="playback-card-artwork" aria-hidden="true">
				{artwork.icon ? (
					<span className="pool-card-icon">{artwork.icon}</span>
				) : artwork.image ? (
					<img className="pool-card-image" src={artwork.image} alt="" />
				) : null}
			</div>
			{artwork.notice && (
				<span className="playback-preview-notice" role="status">
					{artwork.notice}
				</span>
			)}
		</>
	);
}
