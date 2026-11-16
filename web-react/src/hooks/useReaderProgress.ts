import { useEffect } from 'react';
import { useComicCacheStore } from '@/stores/comicCache.store';

export function useReaderProgress(uid: string | undefined, pages: string[], currentPage: number) {
    useEffect(() => {
        if (!uid || pages.length === 0) return;
        const total = pages.length || 1;
        const per = (currentPage / total) * 100;
        useComicCacheStore.getState().setCacheById(uid, {
            readPer: Number(per.toFixed(2)),
            currentPage: currentPage,
            read: per === 100,
        });
    }, [uid, pages, currentPage]);
}
