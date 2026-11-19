import { ultraCompState } from 'ultra-light-js';

const DESKTOP_QUERY = '(min-width: 801px)';

const mediaQuery = typeof window !== 'undefined' ? window.matchMedia(DESKTOP_QUERY) : null;

export const VIEWPORT_CONTEXT = ultraCompState({
    isDesktop: mediaQuery?.matches ?? false,
});

mediaQuery?.addEventListener('change', (event) => {
    VIEWPORT_CONTEXT.isDesktop.set(event.matches);
});
