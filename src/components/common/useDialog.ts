import { useEffect, useRef } from "react";

/** Focus stays inside a modal and returns to its opener on close. */
export function useDialog(onCancel: () => void, busy = false) {
  const panel = useRef<HTMLFormElement & HTMLDivElement>(null);
  const cancel = useRef(onCancel); cancel.current = onCancel;
  const blocked = useRef(busy); blocked.current = busy;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const node = panel.current;
    const controls = () => Array.from(node?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]') ?? []).filter(element => !element.closest('[hidden], [inert], fieldset:disabled'));
    (controls()[0] ?? node)?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); if (!blocked.current) cancel.current(); }
      if (event.key === "Tab") {
        const elements = controls();
        if (!elements.length) { event.preventDefault(); node?.focus(); return; }
        const first = elements[0], last = elements[elements.length - 1];
        if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && (document.activeElement === last || document.activeElement === node)) { event.preventDefault(); first.focus(); }
      }
    };
    node?.addEventListener("keydown", key);
    return () => { node?.removeEventListener("keydown", key); if (previous?.isConnected) previous.focus(); };
  }, []);
  return panel;
}
