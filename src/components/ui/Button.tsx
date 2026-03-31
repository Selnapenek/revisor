import { type JSX, splitProps } from "solid-js";

interface ButtonProps extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md" | "lg";
  loading?: boolean;
}

export function Button(props: ButtonProps) {
  const [local, rest] = splitProps(props, [
    "variant",
    "size",
    "loading",
    "children",
    "class",
    "disabled",
  ]);

  const variant = () => local.variant ?? "primary";
  const size = () => local.size ?? "md";

  const baseClasses =
    "inline-flex items-center justify-center font-medium rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-offset-1 focus:ring-(--color-accent-emphasis) disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer";

  const variantClasses = () => {
    switch (variant()) {
      case "primary":
        return "bg-(--color-accent-emphasis) text-white hover:bg-(--color-accent-fg)";
      case "secondary":
        return "bg-(--color-bg-tertiary) text-(--color-text-primary) border border-(--color-border-default) hover:bg-(--color-bg-overlay)";
      case "ghost":
        return "text-(--color-text-secondary) hover:text-(--color-text-primary) hover:bg-(--color-bg-tertiary)";
      case "danger":
        return "bg-(--color-danger-emphasis) text-white hover:opacity-90";
    }
  };

  const sizeClasses = () => {
    switch (size()) {
      case "sm":
        return "text-xs px-2 py-1 gap-1";
      case "md":
        return "text-sm px-3 py-1.5 gap-1.5";
      case "lg":
        return "text-sm px-4 py-2 gap-2";
    }
  };

  return (
    <button
      class={`${baseClasses} ${variantClasses()} ${sizeClasses()} ${local.class ?? ""}`}
      disabled={local.disabled || local.loading}
      {...rest}
    >
      {local.loading && <Spinner size={size() === "sm" ? 12 : 14} />}
      {local.children}
    </button>
  );
}

function Spinner(props: { size: number }) {
  return (
    <svg
      width={props.size}
      height={props.size}
      viewBox="0 0 24 24"
      fill="none"
      class="animate-spin"
    >
      <circle
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        stroke-width="3"
        stroke-linecap="round"
        opacity="0.25"
      />
      <path
        d="M12 2a10 10 0 0 1 10 10"
        stroke="currentColor"
        stroke-width="3"
        stroke-linecap="round"
      />
    </svg>
  );
}
