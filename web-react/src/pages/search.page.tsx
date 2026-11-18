import { useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import styles from './search-page.module.css';
import { useLibraryStore } from '@/stores/library.store';
import { ItemsGrid } from '@/components/items-grid/items-grid';
import { Layout } from '@/layout';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import type { ILibraryResponseItem } from '@/library.types';
import { applySort, useSort } from '@/hooks/useFilters';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { matchesReadFilter, useReadTypesContext } from '@/context/ReadTypesContext.hooks';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import { ItemCounter } from '@/components/item-counter/item-counter';
import { StateFilter } from '@/components/state-filter/state-filter';
import { LayoutSelector } from '@/components/layout/layout-selector';

export function SearchPage({ search }: { search: string }) {
    const navigate = useNavigate();
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

    const { sort } = useSort();
    const rawItems = useMemo(() => getSearchItems(), [groups, searchQuery]);
    const items = useMemo(() => applySort(rawItems, sort), [rawItems, sort]);

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

    const visibleCount = items.filter((item) =>
        matchesReadFilter(readFilter, comicCache[item.uid]?.readPer || 0),
    ).length;

    return (
        <Layout>
            <section className={styles.page}>
                <header className={styles.header}>
                    <div className={styles.left}>
                        <button
                            type="button"
                            className={styles.backButton}
                            onClick={() => navigate('/')}
                            aria-label="Back to library"
                        >
                            <ArrowLeftIcon size={16} />
                        </button>

                        <div className={styles.summary}>
                            <span className={styles.eyebrow}>Search</span>
                            <h1 className={styles.title}>{`"${search}"`}</h1>
                        </div>

                        <ItemCounter className={styles.counter} count={visibleCount} />
                    </div>

                    <div className={styles.right}>
                        <StateFilter />
                        <LayoutSelector />
                    </div>
                </header>

                {visibleCount === 0 ? (
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
