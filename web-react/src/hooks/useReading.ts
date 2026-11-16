import { useEffect, useMemo, useState } from 'react';
import { getReading } from '@/services/library.service';
import { useLibraryStore } from '@/stores/library.store';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { matchesReadFilter } from '@/context/ReadTypesContext.hooks';
import type { ILibraryResponseItem } from '@/library.types';

export function useReading() {
    const [items, setItems] = useState<ILibraryResponseItem[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const lastDeleted = useLibraryStore((s) => s.lastDeleted);
    const comicCache = useComicCacheStore((s) => s.cache);

    useEffect(() => {
        let isCurrent = true;

        useComicCacheStore
            .getState()
            .flushPending()
            .then(() => getReading())
            .then((data) => {
                if (isCurrent) setItems(data.items);
            })
            .catch(() => {
                if (isCurrent) setError('Comics in progress could not be loaded.');
            })
            .finally(() => {
                if (isCurrent) setIsLoading(false);
            });

        return () => {
            isCurrent = false;
        };
    }, []);

    useEffect(() => {
        if (!lastDeleted) return;
        setItems((prev) => prev.filter((item) => item.uid !== lastDeleted.uid));
    }, [lastDeleted]);

    const inProgress = useMemo(
        () =>
            items.filter((item) =>
                matchesReadFilter('reading', comicCache[item.uid]?.readPer ?? 0),
            ),
        [items, comicCache],
    );

    return { items: inProgress, isLoading, error };
}
