import { useEffect, useRef, useState } from 'react';
import { getRecentlyAdded } from '@/services/library.service';
import { useLibraryStore } from '@/stores/library.store';
import type { ILibraryResponseItem } from '@/library.types';

export function useRecentlyAdded(windowHours: number) {
    const [items, setItems] = useState<ILibraryResponseItem[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const requestId = useRef(0);
    const lastDeleted = useLibraryStore((s) => s.lastDeleted);

    useEffect(() => {
        const currentRequest = ++requestId.current;
        setIsLoading(true);
        setError(null);

        getRecentlyAdded(windowHours)
            .then((data) => {
                if (requestId.current !== currentRequest) return;
                setItems(data.items);
            })
            .catch(() => {
                if (requestId.current === currentRequest)
                    setError('Recently added comics could not be loaded.');
            })
            .finally(() => {
                if (requestId.current === currentRequest) setIsLoading(false);
            });
    }, [windowHours]);

    // A delete goes through the library store. This view does not read that store for anything else.
    // Remove the deleted entry from the local list, so its card leaves the /new view without a reload.
    useEffect(() => {
        if (!lastDeleted) return;
        setItems((prev) => prev.filter((item) => item.uid !== lastDeleted.uid));
    }, [lastDeleted]);

    return { items, isLoading, error };
}
