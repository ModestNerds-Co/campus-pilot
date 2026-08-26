import { useEffect, useRef, useState } from "react";

const DESKTOP_NAVIGATION_QUERY = "(min-width: 1024px)";

export function useNavigationDrawer(
  open: boolean,
  onOpenChange: (open: boolean) => void,
) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const navigationRef = useRef<HTMLElement>(null);
  const [desktopNavigation, setDesktopNavigation] = useState(() =>
    typeof window !== "undefined" && window.matchMedia(DESKTOP_NAVIGATION_QUERY).matches,
  );

  useEffect(() => {
    const media = window.matchMedia(DESKTOP_NAVIGATION_QUERY);
    const updateNavigationMode = () => setDesktopNavigation(media.matches);
    updateNavigationMode();
    media.addEventListener("change", updateNavigationMode);
    return () => media.removeEventListener("change", updateNavigationMode);
  }, []);

  useEffect(() => {
    if (desktopNavigation && open) onOpenChange(false);
  }, [desktopNavigation, onOpenChange, open]);

  useEffect(() => {
    const navigation = navigationRef.current;
    if (!navigation) return;
    if (!desktopNavigation && !open) navigation.setAttribute("inert", "");
    else navigation.removeAttribute("inert");
  }, [desktopNavigation, open]);

  useEffect(() => {
    if (!open || desktopNavigation) return;

    const navigation = navigationRef.current;
    const focusReturnTarget = triggerRef.current;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const focusable = () =>
      Array.from(
        navigation?.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      ).filter((element) => element.offsetParent !== null);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onOpenChange(false);
        return;
      }
      if (event.key !== "Tab") return;

      const items = focusable();
      if (items.length === 0) {
        event.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus({ preventScroll: true });
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus({ preventScroll: true });
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    const focusFrame = window.requestAnimationFrame(() =>
      focusable()[0]?.focus({ preventScroll: true }),
    );

    return () => {
      window.cancelAnimationFrame(focusFrame);
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", handleKeyDown);
      if (focusReturnTarget?.isConnected) {
        window.requestAnimationFrame(() =>
          focusReturnTarget.focus({ preventScroll: true }),
        );
      }
    };
  }, [desktopNavigation, onOpenChange, open]);

  return { desktopNavigation, navigationRef, triggerRef };
}
