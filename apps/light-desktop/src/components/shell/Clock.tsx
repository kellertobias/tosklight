import { useEffect, useState } from "react";

export function Clock({ now: fixedNow }: { now?: Date } = {}) {
	const [liveNow, setLiveNow] = useState(() => new Date());
	useEffect(() => {
		if (fixedNow) return;
		const timer = window.setInterval(() => setLiveNow(new Date()), 1000);
		return () => clearInterval(timer);
	}, [fixedNow]);
	const now = fixedNow ?? liveNow;
	const hour = String(now.getHours()).padStart(2, "0");
	const minute = String(now.getMinutes()).padStart(2, "0");
	return (
		<time className="dock-clock">
			<span>{hour}</span>
			<i>
				<small>{String(now.getSeconds()).padStart(2, "0")}</small>
			</i>
			<span>{minute}</span>
		</time>
	);
}
