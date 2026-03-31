import type { JSX } from "solid-js";

interface BadgeProps {
  variant?: "default" | "success" | "danger" | "warning" | "info";
  children: JSX.Element;
  class?: string;
}

export function Badge(props: BadgeProps) {
  const variant = () => props.variant ?? "default";

  const variantClasses = () => {
    switch (variant()) {
      case "success":
        return "bg-(--color-success-emphasis)/20 text-(--color-success-fg)";
      case "danger":
        return "bg-(--color-danger-emphasis)/20 text-(--color-danger-fg)";
      case "warning":
        return "bg-(--color-warning-fg)/20 text-(--color-warning-fg)";
      case "info":
        return "bg-(--color-accent-emphasis)/20 text-(--color-accent-fg)";
      default:
        return "bg-(--color-bg-tertiary) text-(--color-text-secondary)";
    }
  };

  return (
    <span
      class={`inline-flex items-center px-2 py-0.5 text-xs font-medium rounded-full ${variantClasses()} ${props.class ?? ""}`}
    >
      {props.children}
    </span>
  );
}
