import { useCallback, useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { getWindowRange } from '@/lib/page-window';

export function useReaderPageTracking(
    uid: string | undefined,
    pages: string[],
    viewerRef: RefObject<HTMLElement | null>,
) {
    const [currentPage, setCurrentPage] = useState(1);
    const [isReady, setIsReady] = useState(false);
    const observerRef = useRef<IntersectionObserver | null>(null);

    const goToPage = useCallback(
        (page: number, block: ScrollLogicalPosition = 'start') => {
            const $page = viewerRef.current?.children[page - 1] as HTMLElement | undefined;
            $page?.scrollIntoView({ block, inline: 'start' });
        },
        [viewerRef],
    );

    useEffect(() => {
        observerRef.current?.disconnect();

        const $section = viewerRef.current;
        if (!uid || !$section) return;

        const savedPage = useComicCacheStore.getState().getCacheById(uid)?.currentPage || 1;
        const range = getWindowRange(pages.length, savedPage);
        if (!range) return;

        setCurrentPage(range.targetInd);
        setIsReady(true);

        const elements = Array.from($section.children) as HTMLElement[];
        const pageOf = new Map<HTMLElement, number>();
        elements.forEach((el, i) => pageOf.set(el, i + 1));

        goToPage(range.targetInd);

        const observer = new IntersectionObserver(
            (entries) => {
                const mostVisible = entries
                    .filter((entry) => entry.isIntersecting)
                    .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
                if (!mostVisible) return;
                const page = pageOf.get(mostVisible.target as HTMLElement);
                if (page) setCurrentPage(page);
            },
            { threshold: [0.25, 0.5, 0.75] },
        );

        elements.forEach(($page) => observer.observe($page));
        observerRef.current = observer;

        return () => observer.disconnect();
    }, [pages, uid, viewerRef, goToPage]);

    return { currentPage, isReady, goToPage };
}
