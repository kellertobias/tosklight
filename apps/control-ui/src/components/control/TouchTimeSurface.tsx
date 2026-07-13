import { VerticalTouchFader } from "./VerticalTouchFader";
export function TouchTimeSurface({ label, value, display, maximum, onChange }: { label: string; value: number; display: string; maximum: number; onChange: (value: number) => void }) {
  return <VerticalTouchFader label={label} value={value} maximum={maximum} display={display} directInput onChange={onChange}/>;
}
