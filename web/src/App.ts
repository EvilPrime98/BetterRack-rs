import { UltraComponent, UltraFragment, UltraRouter } from 'ultra-light-js';
import { LibraryPage } from './pages/library-page';
import { ReaderPage } from './pages/reader.page';
import { SettingsPage } from './pages/settings.page';
import { StorePage } from './pages/store.page';
import { StoreDownloadsPage } from './pages/store-downloads.page';
import { RecentPage } from './pages/recent-page';
import { ReadingPage } from './pages/reading-page';
import { COMIC_CACHE_CONTEXT } from './context/comic-cache.context';
import { USER_PREF } from './context/user-pref-cache.context';
import { COMICS_TYPE_CTX } from './context/comics-types.context';
import { SETTINGS_CONTEXT } from './context/settings.context';
import { LIBRARY_CONTEXT } from './context/library.context';
import { AppLoader } from './components/app-loader/app-loader';
import { APP_CTX } from './context/app.context';
import { ServerModal } from './components/server-modal/server-modal';
import { SERVER_MODAL_CTX } from './context/server-modal.context';

export function App() {
    async function onMount() {
        await SERVER_MODAL_CTX.ensureServerConfigured();
        APP_CTX.isLoading.set(true);
        await Promise.all([
            Promise.allSettled([
                LIBRARY_CONTEXT.fetchLibrary(),
                COMIC_CACHE_CONTEXT.init(),
                SETTINGS_CONTEXT.fetchSettings(),
            ]),
        ]);
        if (
            SETTINGS_CONTEXT.settings.get().rescanOnStartup &&
            LIBRARY_CONTEXT.groups.get().length > 0
        ) {
            await LIBRARY_CONTEXT.identifyLibrary();
        }
        USER_PREF.init();
        COMICS_TYPE_CTX.init();
        APP_CTX.isLoading.set(false);
    }

    function onAppLoad($loader: HTMLElement) {
        if (APP_CTX.isLoading.get()) return;
        $loader.after(
            UltraRouter(
                { path: '/:uid/reader', component: ({ uid } = {}) => ReaderPage({ uid }) },
                { path: '/settings', component: () => SettingsPage() },
                { path: '/store/downloads', component: () => StoreDownloadsPage() },
                { path: '/store', component: () => StorePage() },
                { path: '/new', component: () => RecentPage() },
                { path: '/reading', component: () => ReadingPage() },
                { path: '/:uid', component: ({ uid } = {}) => LibraryPage({ uid }) },
                { path: '/*', component: () => LibraryPage({}) },
            ),
        );
    }

    function onIdentifyProgress($loader: HTMLElement) {
        const progress = LIBRARY_CONTEXT.identifyProgress.get();
        const $hint = $loader.querySelector('p:last-of-type');
        if (!$hint) return;
        $hint.textContent = progress
            ? `Identifying library${progress.total ? ` ${progress.done}/${progress.total}` : '…'}`
            : 'Loading your library…';
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
