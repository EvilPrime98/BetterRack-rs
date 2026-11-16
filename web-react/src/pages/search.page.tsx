import { useEffect, useMemo } from 'react';
import styles from './search-page.module.css';
import { PageHeader } from '@/components/page-header/page-header';
import { useLibraryStore } from '@/stores/library.store';
import { ItemsGrid } from '@/components/items-grid/items-grid';
import { Layout } from '@/layout';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import type { ILibraryResponseItem } from '@/library.types';
import { applyFilters, useFilters } from '@/hooks/useFilters';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { matchesReadFilter, useReadTypesContext } from '@/context/ReadTypesContext.hooks';
import { useComicCacheStore } from '@/stores/comicCache.store';

export function SearchPage({ search }: { search: string }) {
    const groups = useLibraryStore((s) => s.groups);
    const searchQuery = useLibraryStore((s) => s.searchQuery);
    const comicsType = useComicsTypeStore((s) => s.type);
    const setTitle = useDocumentTitleStore((s) => s.setTitle);

    const { type: readFilter } = useReadTypesContext();
    const comicCache = useComicCacheStore((s) => s.cache);

    function getSearchItems(): ILibraryResponseItem[] {
        const query = useLibraryStore.getState().searchQuery.trim().toLowerCase();
        return useLibraryStore
            .getState()
            .getLibraryItems({ onlyDir: false })
            .filter((item) => item.did === false)
            .filter((item) => item.name.toLowerCase().includes(query));
    }

    const { filters, setFilters, resetFilters } = useFilters();
    const rawItems = useMemo(() => getSearchItems(), [groups, searchQuery]);
    const items = useMemo(() => applyFilters(rawItems, filters), [rawItems, filters]);

    useEffect(() => {
        if (useLibraryStore.getState().searchQuery !== search) {
            useLibraryStore.setState({ searchQuery: search });
        }
    }, [search]);

    useEffect(() => {
        setTitle(`Search: "${search}"`);
    }, [search, setTitle]);

    useEffect(() => {
        useLibraryStore.getState().fetchLibrary();
    }, []);

    const hasVisibleItems = items.some((item) =>
        matchesReadFilter(readFilter, comicCache[item.uid]?.readPer || 0),
    );

    return (
        <Layout>
            <section className={styles.page}>
                <PageHeader
                    items={items}
                    filters={filters}
                    setFilters={setFilters}
                    resetFilters={resetFilters}
                />

                {!hasVisibleItems ? (
                    <p className={styles.empty}>No items to show.</p>
                ) : (
                    <section
                        className={[
                            styles.comicContainer,
                            comicsType === 'detail' ? styles.detailLayout : '',
                        ]
                            .filter(Boolean)
                            .join(' ')}
                    >
                        <ItemsGrid key={search} items={items} />
                    </section>
                )}
            </section>
        </Layout>
    );
}
