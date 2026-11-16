import { useEffect, useState } from 'react';

const DESKTOP_QUERY = '(min-width: 801px)';

/**
 * Tracks whether the viewport is wider than 800px, i.e. the breakpoint at which
 * the sidebar becomes a permanent part of the layout instead of an overlay.
 */
export function useIsDesktop(): boolean {
    const [isDesktop, setIsDesktop] = useState(
        () => typeof window !== 'undefined' && window.matchMedia(DESKTOP_QUERY).matches,
    );

    useEffect(() => {
        const mq = window.matchMedia(DESKTOP_QUERY);
        function onChange(event: MediaQueryListEvent) {
            setIsDesktop(event.matches);
        }
        mq.addEventListener('change', onChange);
        return () => mq.removeEventListener('change', onChange);
    }, []);

    return isDesktop;
}
