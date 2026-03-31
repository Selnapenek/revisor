import { createMemo } from "solid-js";
import { marked } from "marked";

// Configure marked for safe, GitHub-flavored rendering
marked.setOptions({
  gfm: true,
  breaks: true,
});

interface MarkdownProps {
  content: string;
  class?: string;
}

export function Markdown(props: MarkdownProps) {
  const html = createMemo(() => {
    try {
      return marked.parse(props.content, { async: false }) as string;
    } catch {
      return props.content;
    }
  });

  return (
    <div
      class={`markdown-body ${props.class ?? ""}`}
      innerHTML={html()}
    />
  );
}
