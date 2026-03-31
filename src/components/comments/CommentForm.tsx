import { createSignal, Show } from "solid-js";
import { Button } from "~/components/ui/Button";
import { Markdown } from "./Markdown";

interface CommentFormProps {
  onSubmit: (body: string) => Promise<void>;
  onCancel: () => void;
  placeholder?: string;
  autofocus?: boolean;
  submitLabel?: string;
  initialValue?: string;
}

export function CommentForm(props: CommentFormProps) {
  const [body, setBody] = createSignal(props.initialValue ?? "");
  const [submitting, setSubmitting] = createSignal(false);
  const [preview, setPreview] = createSignal(false);

  const handleSubmit = async () => {
    const text = body().trim();
    if (!text) return;
    setSubmitting(true);
    try {
      await props.onSubmit(text);
      setBody("");
      setPreview(false);
    } finally {
      setSubmitting(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      handleSubmit();
    }
    if (e.key === "Escape") {
      props.onCancel();
    }
  };

  return (
    <div class="flex flex-col gap-2">
      {/* Write / Preview tabs */}
      <div class="flex gap-2 border-b border-(--color-border-muted) pb-1">
        <button
          class={`text-xs px-1 pb-1 cursor-pointer transition-colors border-b-2 ${
            !preview()
              ? "border-(--color-accent-emphasis) text-(--color-text-primary)"
              : "border-transparent text-(--color-text-muted) hover:text-(--color-text-secondary)"
          }`}
          onClick={() => setPreview(false)}
        >
          Write
        </button>
        <button
          class={`text-xs px-1 pb-1 cursor-pointer transition-colors border-b-2 ${
            preview()
              ? "border-(--color-accent-emphasis) text-(--color-text-primary)"
              : "border-transparent text-(--color-text-muted) hover:text-(--color-text-secondary)"
          }`}
          onClick={() => setPreview(true)}
        >
          Preview
        </button>
      </div>

      <Show
        when={!preview()}
        fallback={
          <div class="min-h-[80px] px-3 py-2 rounded border border-(--color-border-default) bg-(--color-bg-primary)">
            <Show
              when={body().trim()}
              fallback={<span class="text-xs text-(--color-text-muted)">Nothing to preview</span>}
            >
              <Markdown content={body()} />
            </Show>
          </div>
        }
      >
        <textarea
          ref={(el) => props.autofocus !== false && setTimeout(() => el.focus(), 0)}
          class="w-full min-h-[80px] px-3 py-2 text-xs font-mono rounded border border-(--color-border-default) bg-(--color-bg-primary) text-(--color-text-primary) placeholder:text-(--color-text-muted) resize-y focus:outline-none focus:ring-2 focus:ring-(--color-accent-emphasis) focus:border-transparent"
          placeholder={props.placeholder ?? "Leave a comment... (Markdown supported)"}
          value={body()}
          onInput={(e) => setBody(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          disabled={submitting()}
        />
      </Show>

      <div class="flex items-center gap-2 justify-between">
        <span class="text-[10px] text-(--color-text-muted)">
          Markdown supported &middot; {"\u2318"}Enter to submit
        </span>
        <div class="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={props.onCancel} disabled={submitting()}>
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={handleSubmit}
            loading={submitting()}
            disabled={!body().trim() || submitting()}
          >
            {props.submitLabel ?? "Comment"}
          </Button>
        </div>
      </div>
    </div>
  );
}
