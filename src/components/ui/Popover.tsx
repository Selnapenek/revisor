import { type JSX, createSignal, onCleanup, Show } from "solid-js";

interface PopoverProps {
  trigger: (props: { ref: (el: HTMLElement) => void; onClick: () => void; isOpen: boolean }) => JSX.Element;
  children: JSX.Element;
  align?: "start" | "end";
}

export function Popover(props: PopoverProps) {
  const [isOpen, setIsOpen] = createSignal(false);
  let triggerEl: HTMLElement | undefined;
  let contentEl: HTMLDivElement | undefined;

  const toggle = () => setIsOpen((prev) => !prev);

  const onDocumentClick = (e: MouseEvent) => {
    const target = e.target as Node;
    if (triggerEl?.contains(target) || contentEl?.contains(target)) return;
    setIsOpen(false);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") setIsOpen(false);
  };

  // Attach/detach global listeners
  const attach = () => {
    document.addEventListener("mousedown", onDocumentClick);
    document.addEventListener("keydown", onKeyDown);
  };
  const detach = () => {
    document.removeEventListener("mousedown", onDocumentClick);
    document.removeEventListener("keydown", onKeyDown);
  };

  // Watch open state to toggle listeners
  const prevOpen = { value: false };
  const checkListeners = () => {
    const open = isOpen();
    if (open && !prevOpen.value) attach();
    if (!open && prevOpen.value) detach();
    prevOpen.value = open;
  };

  // Use a getter-based approach: call checkListeners whenever rendering
  onCleanup(detach);

  return (
    <div class="relative">
      {props.trigger({
        ref: (el: HTMLElement) => { triggerEl = el; },
        onClick: toggle,
        isOpen: isOpen(),
      })}
      <Show when={(() => { checkListeners(); return isOpen(); })()}>
        <div
          ref={contentEl!}
          class={`absolute top-full mt-1 z-50 min-w-[220px] bg-(--color-bg-secondary) border border-(--color-border-default) rounded-lg shadow-lg overflow-hidden ${
            (props.align ?? "start") === "end" ? "right-0" : "left-0"
          }`}
        >
          {props.children}
        </div>
      </Show>
    </div>
  );
}
