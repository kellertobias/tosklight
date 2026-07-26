import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "../controls";
import {
	InputModal,
	ModalNumberInput,
	ModalNumberValue,
	ModalTextKeyboard,
} from "../input";

interface InputStoryArgs {
	initialValue?: string;
	replaceOnFirstInput?: boolean;
	allowDecimal?: boolean;
	allowThrough?: boolean;
	kind?: "text" | "multiline" | "number";
}

const meta = {
	title: "Controls/Keyboard and numpad",
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: { source: { type: "dynamic" } },
	},
	args: {
		initialValue: "62.8",
		replaceOnFirstInput: true,
		allowDecimal: true,
		allowThrough: true,
		kind: "number",
	},
	argTypes: {
		initialValue: { control: "text" },
		replaceOnFirstInput: { control: "boolean" },
		allowDecimal: { control: "boolean" },
		allowThrough: { control: "boolean" },
		kind: { control: "inline-radio", options: ["text", "multiline", "number"] },
	},
} satisfies Meta<InputStoryArgs>;

export default meta;
type Story = StoryObj<InputStoryArgs>;

function NumberPadExample(args: Required<InputStoryArgs>) {
	const [value, setValue] = useState(args.initialValue);
	const [caret, setCaret] = useState(args.initialValue.length);
	const [status, setStatus] = useState("Editing");
	return (
		<section style={{ width: 620 }}>
			<ModalNumberValue
				value={value}
				caret={caret}
				onCaretChange={setCaret}
				ariaLabel="Current number"
			/>
			<small>{status}</small>
			<ModalNumberInput
				value={value}
				caret={caret}
				onChange={setValue}
				onCaretChange={setCaret}
				onEnter={() => setStatus("Entered")}
				onEscape={() => setStatus("Cancelled")}
				replaceOnFirstInput={args.replaceOnFirstInput}
				allowDecimal={args.allowDecimal}
				allowThrough={args.allowThrough}
			/>
		</section>
	);
}

export const NumberPad: Story = {
	render: (args) => <NumberPadExample {...meta.args} {...args} />,
};

export const IntegerNumberPad: Story = {
	args: {
		initialValue: "512",
		replaceOnFirstInput: false,
		allowDecimal: false,
		allowThrough: false,
	},
	render: (args) => <NumberPadExample {...meta.args} {...args} />,
};

export const ThroughExpression: Story = {
	args: {
		initialValue: "0 THRU 100",
		replaceOnFirstInput: false,
		allowDecimal: true,
		allowThrough: true,
	},
	render: (args) => <NumberPadExample {...meta.args} {...args} />,
};

function KeyboardExample({ multiline = false }: { multiline?: boolean }) {
	const [value, setValue] = useState(
		multiline ? "Fixture notes\nSecond line" : "Fixture",
	);
	const [status, setStatus] = useState("Editing");
	return (
		<section>
			<output
				aria-label="Current text"
				style={{
					display: "block",
					marginBottom: 8,
					fontSize: 20,
					color: "var(--cyan)",
				}}
			>
				{value}
			</output>
			<small>{status}</small>
			<ModalTextKeyboard
				value={value}
				onChange={setValue}
				onEnter={() => setStatus("Confirmed")}
				onEscape={() => setStatus("Cancelled")}
				actionLabel={multiline ? "Save notes" : "Confirm"}
				multiline={multiline}
			/>
		</section>
	);
}

export const Keyboard: Story = {
	render: () => <KeyboardExample />,
};

export const MultilineKeyboard: Story = {
	render: () => <KeyboardExample multiline />,
};

function InputModalExample({
	kind,
	initialValue,
	allowDecimal,
	placeholder,
}: Required<Pick<InputStoryArgs, "kind" | "initialValue" | "allowDecimal">> & {
	placeholder?: string;
}) {
	const [open, setOpen] = useState(true);
	const [result, setResult] = useState(initialValue);
	return (
		<section style={{ minHeight: 640 }}>
			<Button onClick={() => setOpen(true)}>Open input modal</Button>
			<output aria-label="Committed input modal value">{result}</output>
			{open && (
				<InputModal
					kind={kind}
					value={result}
					allowDecimal={allowDecimal}
					label={kind === "number" ? "Fade time" : "Fixture name"}
					placeholder={placeholder}
					unit={kind === "number" ? "s" : undefined}
					onCommit={(value) => {
						setResult(value);
						setOpen(false);
					}}
					onCancel={() => setOpen(false)}
				/>
			)}
		</section>
	);
}

export const InputModalConfigurations: Story = {
	render: (args) => (
		<InputModalExample
			kind={args.kind ?? "number"}
			initialValue={args.initialValue ?? "2.5"}
			allowDecimal={args.allowDecimal ?? true}
		/>
	),
};

export const EmptyTextInputModal: Story = {
	render: () => (
		<InputModalExample
			kind="text"
			initialValue=""
			allowDecimal={false}
			placeholder="Enter fixture name"
		/>
	),
};

export const MultilineInputModal: Story = {
	render: () => (
		<InputModalExample
			kind="multiline"
			initialValue={[
				"First line",
				"Second line",
				"Third line",
				"Fourth line",
				"Fifth line",
				"Sixth line",
				"Seventh line",
				"Eighth line",
			].join("\n")}
			allowDecimal={false}
			placeholder="Add operator notes"
		/>
	),
};
