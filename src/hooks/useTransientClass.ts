import { useEffect, useRef } from "react";

/** Toggles `className` on the returned ref each time `pulse` increments.
 *  The class is force-reflowed off-then-on so consecutive pulses always
 *  restart the underlying CSS animation. `pulse === 0` is treated as the
 *  initial render and never fires. */
export function useTransientClass<T extends HTMLElement>(
  pulse: number,
  className: string,
  durationMs: number,
) {
  const ref = useRef<T>(null);
  useEffect(() => {
    if (pulse === 0 || !ref.current) return;
    const el = ref.current;
    el.classList.remove(className);
    // Force reflow so the next add restarts the animation.
    void el.offsetWidth;
    el.classList.add(className);
    const id = window.setTimeout(() => el.classList.remove(className), durationMs);
    return () => clearTimeout(id);
  }, [pulse, className, durationMs]);
  return ref;
}
