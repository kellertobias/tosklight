interface FixtureColorDotProps {
  color: string;
}

export function FixtureColorDot({ color }: FixtureColorDotProps) {
  return <i aria-hidden="true" className="color-dot" style={{ background: color, border: "1px solid #a5afb6" }} />;
}
