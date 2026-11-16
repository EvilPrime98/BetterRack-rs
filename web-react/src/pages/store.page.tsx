import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import styles from './store.page.module.css';
import { Layout } from '@/layout';
import { StoreCard } from '@/components/store-card/store-card';
import { searchComics, getLatestComics } from '@/services/store.service';
import { toast } from '@/services/toast.service';
import type { IStorePost } from '@/store.types';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { useStorePageStore } from '@/stores/store.store';
import { BRButton } from '@/components/br-button/br-button';

const PAGE_SIZE = 30;

function keyFor(item: IStorePost) {
    return item.id !== undefined ? `id:${item.id}` : `link:${item.link}`;
}

export function StorePage() {
    const setTitle = useDocumentTitleStore((s) => s.setTitle);

    const query = useStorePageStore((s) => s.query);
    const results = useStorePageStore((s) => s.results);
    const setStoreState = useStorePageStore((s) => s.set);

    const [isLoading, setIsLoading] = useState(false);
    const [isLoadingMore, setIsLoadingMore] = useState(false);
    const [error, setError] = useState('');

    const loadingRef = useRef({ isLoading, isLoadingMore });
    useEffect(() => {
        loadingRef.current = { isLoading, isLoadingMore };
    });

    const fetchPage = useCallback(async (pageNum: number, term: string): Promise<IStorePost[]> => {
        return term
            ? searchComics({ search: term, page: pageNum, perPage: PAGE_SIZE })
            : getLatestComics({ page: pageNum, perPage: PAGE_SIZE });
    }, []);

    const loadFirstPage = useCallback(
        async (term: string) => {
            setIsLoading(true);
            setError('');
            try {
                const items = await fetchPage(1, term);
                setStoreState({
                    results: items,
                    page: 1,
                    hasMore: items.length === PAGE_SIZE,
                    hasLoaded: true,
                });
            } catch (e) {
                const message = e instanceof Error ? e.message : 'Failed to load comics.';
                setError(message);
                toast.error(message);
            } finally {
                setIsLoading(false);
            }
        },
        [fetchPage, setStoreState],
    );

    const loadMore = useCallback(async () => {
        const cached = useStorePageStore.getState();
        const { isLoading, isLoadingMore } = loadingRef.current;
        if (!cached.hasMore || isLoading || isLoadingMore) return;
        const nextPage = cached.page + 1;
        setIsLoadingMore(true);
        try {
            const items = await fetchPage(nextPage, cached.activeSearch);
            setStoreState({
                results: [...useStorePageStore.getState().results, ...items],
                page: nextPage,
                hasMore: items.length === PAGE_SIZE,
            });
        } catch (e) {
            const message = e instanceof Error ? e.message : 'Failed to load more comics.';
            toast.error(message);
        } finally {
            setIsLoadingMore(false);
        }
    }, [fetchPage, setStoreState]);

    function runSearch() {
        const term = query.trim();
        setStoreState({ activeSearch: term });
        loadFirstPage(term);
    }

    useEffect(() => {
        setTitle('Store');
        if (!useStorePageStore.getState().hasLoaded) loadFirstPage('');
    }, [setTitle, loadFirstPage]);

    useLayoutEffect(() => {
        const savedScrollY = useStorePageStore.getState().scrollY;
        if (savedScrollY > 0) window.scrollTo(0, savedScrollY);
        return () => useStorePageStore.setState({ scrollY: window.scrollY });
    }, []);

    const sentinelRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const $sentinel = sentinelRef.current;
        if (!$sentinel) return;
        const observer = new IntersectionObserver(
            (entries) => {
                if (!entries.some((e) => e.isIntersecting)) return;
                loadMore();
            },
            { rootMargin: '600px' },
        );
        observer.observe($sentinel);
        return () => observer.disconnect();
    }, [loadMore]);

    const isEmpty = !isLoading && results.length === 0;

    return (
        <Layout>
            <section className={styles.page}>
                <h1 className={styles.title}>Store</h1>

                <div className={styles.searchRow}>
                    <input
                        type="text"
                        className={styles.searchInput}
                        placeholder="Search for a comic…"
                        aria-label="Search comics"
                        value={query}
                        onChange={(e) => setStoreState({ query: e.target.value })}
                        onKeyDown={(e) => {
                            if (e.key === 'Enter') runSearch();
                        }}
                    />

                    <BRButton text="Search" onClick={runSearch}></BRButton>
                </div>

                <p className={styles.errorText}>{error}</p>

                <p className={styles.empty} style={{ display: isEmpty ? undefined : 'none' }}>
                    {isLoading ? 'Loading…' : 'No comics found.'}
                </p>

                <section className={styles.grid}>
                    {results.map((item) => (
                        <StoreCard key={keyFor(item)} item={item} />
                    ))}
                    <div className={styles.sentinel} ref={sentinelRef} />
                    {isLoadingMore && <p className={styles.loadingMore}>Loading more…</p>}
                </section>
            </section>
        </Layout>
    );
}
