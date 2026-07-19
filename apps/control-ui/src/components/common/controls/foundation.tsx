import {
  createContext,
  forwardRef,
  useContext,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type ReactNode,
} from "react";

export type ControlSize = "default" | "compact";
export type ButtonVariant =
  | "primary"
  | "secondary"
  | "ghost"
  | "danger"
  | "success"
  | "warning";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ControlSize;
  active?: boolean;
  loading?: boolean;
  fullWidth?: boolean;
  iconOnly?: boolean;
}

function buttonClassName(props: Required<Pick<ButtonProps,
  "variant" | "size" | "active" | "fullWidth" | "iconOnly"
>> & { arrowOnly: boolean; className: string }) {
  return [
    "ui-button",
    `ui-${props.variant}`,
    `ui-${props.size}`,
    props.active && "is-active",
    props.fullWidth && "is-full-width",
    props.iconOnly && "is-icon-only",
    props.arrowOnly && "is-arrow-only",
    props.className,
  ].filter(Boolean).join(" ");
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    variant = "secondary", size = "default", active = false, loading = false,
    fullWidth = false, iconOnly = false, className = "", disabled, children,
    type = "button", ...props
  },
  ref,
) {
  const arrowOnly = typeof children === "string" && /^[←→▲▼‹›]$/.test(children.trim());
  const classes = buttonClassName({
    variant, size, active, fullWidth, iconOnly, arrowOnly, className,
  });
  return (
    <button {...props} ref={ref} type={type} disabled={disabled || loading}
      aria-busy={loading || undefined} className={classes}>
      {loading ? <><span className="ui-spinner" aria-hidden="true"/>Loading</> : children}
    </button>
  );
});

export type LabelPlacement = "side" | "top";

const FormLayoutContext = createContext<LabelPlacement>("top");

export interface FormLayoutProps {
  labelPlacement?: LabelPlacement;
  columns?: number;
  minColumnWidth?: number;
  labelWidth?: number;
  className?: string;
  children: ReactNode;
}

export function FormLayout({
  labelPlacement = "top", columns = 1, minColumnWidth = 240,
  labelWidth = 150, className = "", children,
}: FormLayoutProps) {
  const style = {
    "--form-columns": columns,
    "--form-column-min": `${minColumnWidth}px`,
    "--form-label-width": `${labelWidth}px`,
  } as CSSProperties;
  return (
    <FormLayoutContext.Provider value={labelPlacement}>
      <div className={`ui-form-layout labels-${labelPlacement} ${className}`.trim()}
        style={style}>{children}</div>
    </FormLayoutContext.Provider>
  );
}

export interface FormFieldProps {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  required?: boolean;
  htmlFor?: string;
  children: ReactNode;
  className?: string;
  labelPlacement?: LabelPlacement;
}

export function FormField({
  label, description, error, required, htmlFor, children, className = "",
  labelPlacement,
}: FormFieldProps) {
  const inherited = useContext(FormLayoutContext);
  const placement = labelPlacement ?? inherited;
  return (
    <div className={`ui-form-field labels-${placement} ${error ? "has-error" : ""} ${className}`.trim()}>
      {label && <label htmlFor={htmlFor}>{label}{required && <span aria-hidden="true"> *</span>}</label>}
      <div className="ui-form-control">{children}</div>
      {description && !error && <small>{description}</small>}
      {error && <small className="ui-field-error" role="alert">{error}</small>}
    </div>
  );
}

/** Compatibility alias for existing callers. New form code should use FormField. */
export const Field = FormField;
