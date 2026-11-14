import { UltraComponent, UltraFragment } from 'ultra-light-js';
import { AppLoader, LOADER_HINT_SELECTOR } from './components/app-loader/app-loader';
import { APP_CTX } from './context/app.context';
import { LIBRARY_CONTEXT } from './context/library.context';
import { SERVER_MODAL_CTX } from './context/server-modal.context';
import { ServerModal } from './components/server-modal/server-modal';
import { bootstrap } from './app/bootstrap';
import { mountRoot } from './app/root-switcher';

async function onMount() {
    await SERVER_MODAL_CTX.ensureServerConfigured();
    APP_CTX.isLoading.set(true);
    try {
        await bootstrap();
    } finally {
        APP_CTX.isLoading.set(false);
    }
}

function identifyHint() {
    const progress = LIBRARY_CONTEXT.identifyProgress.get();
    if (!progress) return 'Loading your library…';
    return `Identifying library${progress.total ? ` ${progress.done}/${progress.total}` : '…'}`;
}

export function App() {
    let disposeRoot: (() => void) | null = null;

    function onAppLoad($loader: HTMLElement) {
        if (APP_CTX.isLoading.get()) return;
        disposeRoot?.();
        disposeRoot = mountRoot($loader);
    }

    function onIdentifyProgress($loader: HTMLElement) {
        const $hint = $loader.querySelector(LOADER_HINT_SELECTOR);
        if ($hint) $hint.textContent = identifyHint();
    }

    return UltraComponent({
        onMount: [onMount],
        component: UltraFragment(),
        children: [
            UltraComponent({
                component: AppLoader({
                    mode: {
                        state: APP_CTX.isLoading.get,
                        subscriber: APP_CTX.isLoading.subscribe,
                    },
                }),
                trigger: [
                    {
                        subscriber: APP_CTX.isLoading.subscribe,
                        triggerFunction: onAppLoad,
                    },
                    {
                        subscriber: LIBRARY_CONTEXT.identifyProgress.subscribe,
                        triggerFunction: onIdentifyProgress,
                    },
                ],
            }),
            ServerModal(),
        ],
    });
}
