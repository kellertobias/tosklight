import { useEffect, useState } from "react";

export function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => { const timer = window.setInterval(() => setNow(new Date()), 1000); return () => clearInterval(timer); }, []);
  const hour = String(now.getHours()).padStart(2, "0");
  const minute = String(now.getMinutes()).padStart(2, "0");
  return <time className="dock-clock"><span>{hour}</span><i><small>{String(now.getSeconds()).padStart(2, "0")}</small></i><span>{minute}</span></time>;
}
