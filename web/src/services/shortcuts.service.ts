import { createShortcutRegistry } from '../lib/shortcuts';
import { toggleFullscreen } from './fullscreen.service';

export const SHORTCUTS = createShortcutRegistry();

export function registerGlobalShortcuts(): () => void {
    const unregister = SHORTCUTS.register([
        { key: 'ArrowLeft', alt: true, handler: () => window.history.back() },
        { key: 'ArrowRight', alt: true, handler: () => window.history.forward() },
        { key: 'F11', handler: toggleFullscreen },
    ]);
    const onKeydown = (e: KeyboardEvent) => void SHORTCUTS.handle(e);
    window.addEventListener('keydown', onKeydown);
    return () => {
        window.removeEventListener('keydown', onKeydown);
        unregister();
    };
}
