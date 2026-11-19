import { useEffect } from 'react';
import { Routes, Route, useParams } from 'react-router-dom';
import { useAppContext } from '@/context/AppContext.hooks';
import { useLibraryStore } from '@/stores/library.store';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import { useSettingsStore } from '@/stores/settings.store';
import { useServerModalStore } from '@/stores/serverModal.store';
import { useBookmarksStore } from '@/stores/bookmarks.store';
import { usePullListStore } from '@/stores/pullList.store';
import { useDownloadedStore } from '@/stores/downloaded.store';
import { PullListPage } from '@/pages/pull-list.page';
import { useUserPrefStore } from '@/stores/userPref.store';
import { AppLoader } from '@/components/app-loader/app-loader';
import { ServerModal } from '@/components/server-modal/server-modal';
import { LibraryPage } from '@/pages/library-page';
import { ReaderPage } from '@/pages/reader.page';
import { SettingsPage } from '@/pages/settings.page';
import { StorePage } from '@/pages/store.page';
import { StoreDownloadsPage } from '@/pages/store-downloads.page';
import { FilterPage } from '@/pages/filtered-page';
import { RecentPage } from '@/pages/recent-page';
import { ReadingPage } from '@/pages/reading-page';

function ReaderRoute() {
    const { uid } = useParams<{ uid: string }>();
    return <ReaderPage key={uid} />;
}

export function App() {
    const { isLoading, setIsLoading } = useAppContext();
    const identifyProgress = useLibraryStore((s) => s.identifyProgress);

    const loaderMessage = identifyProgress
        ? `Identifying library${identifyProgress.total ? ` ${identifyProgress.done}/${identifyProgress.total}` : '…'}`
        : undefined;

    useEffect(() => {
        (async () => {
            await useServerModalStore.getState().ensureServerConfigured();

            setIsLoading(true);

            try {
                await Promise.allSettled([
                    useLibraryStore.getState().fetchLibrary(),
                    useComicCacheStore.getState().init(),
                    useSettingsStore.getState().fetchSettings(),
                    useBookmarksStore.getState().load(),
                    usePullListStore.getState().load(),
                    useDownloadedStore.getState().load(),
                ]);

                if (
                    useSettingsStore.getState().settings.rescanOnStartup &&
                    useLibraryStore.getState().groups.length > 0
                ) {
                    await useLibraryStore.getState().identifyLibrary();
                }

                useUserPrefStore.getState().init();
                useComicsTypeStore.getState().init();
            } finally {
                setIsLoading(false);
            }
        })();
    }, [setIsLoading]);

    return (
        <>
            <AppLoader visible={isLoading} message={loaderMessage} />

            {!isLoading && (
                <Routes>
                    <Route path="/:uid/reader" element={<ReaderRoute />} />
                    <Route path="/settings" element={<SettingsPage />} />
                    <Route path="/store/downloads" element={<StoreDownloadsPage />} />
                    <Route path="/store/pull-list" element={<PullListPage />} />
                    <Route path="/store" element={<StorePage />} />
                    <Route path="/filters" element={<FilterPage />} />
                    <Route path="/new" element={<RecentPage />} />
                    <Route path="/reading" element={<ReadingPage />} />
                    <Route path="/:uid" element={<LibraryPage />} />
                    <Route path="*" element={<LibraryPage />} />
                </Routes>
            )}

            <ServerModal />
        </>
    );
}
