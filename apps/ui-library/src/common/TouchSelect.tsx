import { SelectField } from "./controls";

export function TouchSelect({ label, value, options, onChange }: { label: string; value: number; options: number[]; onChange: (value: number) => void }) {
  return <SelectField label={label} value={String(value)} options={options.map((option)=>({value:String(option),label:option}))} onChange={(next)=>onChange(Number(next))}/>;
}
