import { type JSX, splitProps } from "solid-js";

interface InputProps extends JSX.InputHTMLAttributes<HTMLInputElement> {
  error?: string;
}

export function Input(props: InputProps) {
  const [local, rest] = splitProps(props, ["error", "class"]);

  return (
    <div class="flex flex-col gap-1">
      <input
        class={`w-full px-3 py-2 text-sm rounded-md border bg-(--color-bg-secondary) text-(--color-text-primary) placeholder:(--color-text-muted) focus:outline-none focus:ring-2 focus:ring-(--color-accent-emphasis) focus:border-transparent transition-colors ${
          local.error
            ? "border-(--color-danger-fg)"
            : "border-(--color-border-default)"
        } ${local.class ?? ""}`}
        {...rest}
      />
      {local.error && (
        <span class="text-xs text-(--color-danger-fg)">{local.error}</span>
      )}
    </div>
  );
}
