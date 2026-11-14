import { Layout } from '../layout';
import { buildReaderRouter, buildShellRouter, isReaderPath } from './routes';

type Root = HTMLElement & { _cleanup?: () => void };

export function mountRoot($anchor: HTMLElement) {
    let $root: Root | null = null;
    let inReader: boolean | null = null;

    function sync() {
        const reader = isReaderPath();
        if ($root && reader === inReader) return;
        $root?._cleanup?.();
        $root?.remove();
        inReader = reader;
        $root = reader ? buildReaderRouter() : Layout(buildShellRouter());
        $anchor.after($root);
    }

    sync();
    window.addEventListener('popstate', sync);

    return function dispose() {
        window.removeEventListener('popstate', sync);
        $root?._cleanup?.();
        $root?.remove();
        $root = null;
    };
}
