import {
  forwardRef,
  useId,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import {
  FormField,
  type ControlSize,
  type LabelPlacement,
} from "./foundation";
import {
  LargeTextInput,
  NumberInput,
  TextInput,
  type LargeTextInputProps,
  type NumberInputProps,
  type TextInputProps,
} from "./textInputs";

type FieldDecoration = {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  labelPlacement?: LabelPlacement;
};

function useFieldId(id?: string) {
  const generated = useId();
  return id ?? generated;
}

type TextFieldProps = TextInputProps & FieldDecoration & { controlSize?: ControlSize };

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  {
    label, description, error, controlSize = "default", id, className = "",
    required, labelPlacement, ...props
  },
  ref,
) {
  const fieldId = useFieldId(id);
  const keyboardLabel = props.keyboardLabel ?? (typeof label === "string" ? label : undefined);
  return <FormField label={label} description={description} error={error} required={required}
    htmlFor={fieldId} labelPlacement={labelPlacement}>
    <TextInput {...props} required={required} ref={ref} id={fieldId}
      aria-invalid={Boolean(error) || undefined}
      className={`ui-${controlSize} ${className}`.trim()} keyboardLabel={keyboardLabel}/>
  </FormField>;
});

type NumberFieldProps = NumberInputProps & FieldDecoration;

export const NumberField = forwardRef<HTMLInputElement, NumberFieldProps>(function NumberField(
  { label, description, error, id, required, className = "", labelPlacement, ...props },
  ref,
) {
  const fieldId = useFieldId(id);
  const keyboardLabel = props.keyboardLabel ?? (typeof label === "string" ? label : undefined);
  return <FormField label={label} description={description} error={error} required={required}
    htmlFor={fieldId} labelPlacement={labelPlacement}>
    <NumberInput {...props} required={required} id={fieldId} className={className}
      keyboardLabel={keyboardLabel} ref={ref}/>
  </FormField>;
});

type TextAreaFieldProps = TextareaHTMLAttributes<HTMLTextAreaElement> & FieldDecoration;

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextAreaFieldProps>(
  function TextAreaField({
    label, description, error, id, className = "", required, labelPlacement, ...props
  }, ref) {
    const fieldId = useFieldId(id);
    return <FormField label={label} description={description} error={error} required={required}
      htmlFor={fieldId} labelPlacement={labelPlacement}>
      <textarea {...props} required={required} ref={ref} id={fieldId}
        aria-invalid={Boolean(error) || undefined}
        className={`ui-textarea ${className}`.trim()}/>
    </FormField>;
  },
);

type LargeTextFieldProps = LargeTextInputProps & FieldDecoration;

export const LargeTextField = forwardRef<HTMLTextAreaElement, LargeTextFieldProps>(
  function LargeTextField({
    label, description, error, id, className = "", required, labelPlacement, ...props
  }, ref) {
    const fieldId = useFieldId(id);
    const keyboardLabel = props.keyboardLabel ?? (typeof label === "string" ? label : undefined);
    return <FormField label={label} description={description} error={error} required={required}
      htmlFor={fieldId} labelPlacement={labelPlacement}>
      <LargeTextInput {...props} required={required} ref={ref} id={fieldId}
        aria-invalid={Boolean(error) || undefined} className={className}
        keyboardLabel={keyboardLabel}/>
    </FormField>;
  },
);
