import type { PropsWithChildren } from "react";
import type { ValueSource } from "../../types";

export function SourceValue({ source, children, className = "" }: PropsWithChildren<{ source: ValueSource; className?: string }>) {
  return <span className={`source-value source-${source} ${className}`}>{children}</span>;
}
