import { useCallback, useEffect, useState } from 'react';
import { reader, readerBookmarks, readerRefresh } from '@/services/library.service';
import type { IBookmark } from '@/library.types';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { preloadWindow } from '@/utils/reader.page.utils';

export function useComicPages(uid: string | undefined) {
    const [pages, setPages] = useState<string[]>([]);
    const [title, setTitle] = useState('');
    const [isLoading, setIsLoading] = useState(true);
    const [hasError, setHasError] = useState(false);
    const [bookmarks, setBookmarks] = useState<IBookmark[]>([]);
    const [isRefreshing, setIsRefreshing] = useState(false);

    const loadPages = useCallback(async () => {
        if (!uid) return;
        setHasError(false);
        setIsLoading(true);
        try {
            await useComicCacheStore.getState().ready();
            const comicCache = useComicCacheStore.getState().getCacheById(uid);
            const data = await reader({ uid });
            const savedPage = comicCache?.currentPage || 1;
            await preloadWindow(uid, data.pages.length, savedPage);
            setTitle(data.title);
            setPages(data.pages);
            // Bookmarks are optional comic metadata. A failure here must not
            // stop the reader from opening.
            try {
                setBookmarks(await readerBookmarks({ uid }));
            } catch {
                setBookmarks([]);
            }
        } catch {
            setHasError(true);
        } finally {
            setIsLoading(false);
        }
    }, [uid]);

    const refreshComic = useCallback(async () => {
        if (!uid || isRefreshing) return;
        setIsRefreshing(true);
        setHasError(false);
        try {
            const data = await readerRefresh({ uid });
            setPages(data);
            try {
                setBookmarks(await readerBookmarks({ uid }));
            } catch {
                setBookmarks([]);
            }
        } catch {
            setHasError(true);
        } finally {
            setIsRefreshing(false);
        }
    }, [uid, isRefreshing]);

    useEffect(() => {
        loadPages();
    }, [loadPages]);

    return { pages, title, isLoading, hasError, bookmarks, isRefreshing, loadPages, refreshComic };
}
