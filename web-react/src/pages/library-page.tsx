import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import styles from './library-page.module.css';
import { PageHeader } from '@/components/page-header/page-header';
import { useLibraryStore } from '@/stores/library.store';
import { ItemsGrid } from '@/components/items-grid/items-grid';
import { Layout } from '@/layout';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import type { ILibraryResponseItem } from '@/library.types';
import { applySort, useSort } from '@/hooks/useFilters';
import { SearchPage } from './search.page';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { matchesReadFilter, useReadTypesContext } from '@/context/ReadTypesContext.hooks';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { useBookmarksStore } from '@/stores/bookmarks.store';
import { bookmarksInFolder } from '@/lib/bookmarks';

export function LibraryPage() {
    const { uid } = useParams<{ uid?: string }>();
    const [searchParams] = useSearchParams();
    const search = searchParams.get('search');
    const groups = useLibraryStore((s) => s.groups);
    const searchQuery = useLibraryStore((s) => s.searchQuery);
    const comicsType = useComicsTypeStore((s) => s.type);
    const setTitle = useDocumentTitleStore((s) => s.setTitle);
    const comicContainerRef = useRef<HTMLElement>(null);
    const { sort, setSort } = useSort();
    const { type: readFilter } = useReadTypesContext();

    const getLibraryItems = useCallback((): ILibraryResponseItem[] => {
        const state = useLibraryStore.getState();
        const items = state.getLibraryItems({ onlyDir: !uid, uid });
        if (!uid && state.structure === 'folders') {
            const rootComics = state.groups
                .flatMap((g) => g.entries)
                .filter((e) => !e.did && !e.parentId);
            items.push(...rootComics);
        }
        const query = useLibraryStore.getState().searchQuery.trim().toLowerCase();
        if (!query) return items;
        return items.filter((item) => item.name.toLowerCase().includes(query));
    }, [uid]);

    const rawItems = useMemo(() => getLibraryItems(), [groups, searchQuery, uid, getLibraryItems]);
    const items = useMemo(() => applySort(rawItems, sort), [rawItems, sort]);

    const structure = useLibraryStore((s) => s.structure);
    const allBookmarks = useBookmarksStore((s) => s.items);
    const bookmarks = useMemo(() => {
        if (structure !== 'folders') return [];
        const query = searchQuery.trim().toLowerCase();
        return bookmarksInFolder(allBookmarks, uid).filter(
            (b) =>
                matchesReadFilter(readFilter, b.read ? 100 : 0) &&
                (!query || b.title.toLowerCase().includes(query)),
        );
    }, [allBookmarks, structure, uid, searchQuery, readFilter]);

    const visibleCount = useComicCacheStore(
        (s) =>
            bookmarks.length +
            items.filter(
                (item) =>
                    item.did || matchesReadFilter(readFilter, s.cache[item.uid]?.readPer || 0),
            ).length,
    );
    const hasVisibleItems = visibleCount > 0;

    useEffect(() => {
        setTitle('Library');
    }, [setTitle]);

    useEffect(() => {
        useLibraryStore.getState().fetchLibrary();
        comicContainerRef.current?.scrollTo(0, 0);
    }, [uid]);

    if (search) {
        return <SearchPage search={search} />;
    }

    return (
        <Layout>
            <section className={styles.page}>
                <PageHeader
                    uid={uid}
                    comicUids={items.filter((i) => !i.did).map((i) => i.uid)}
                    hasSubfolders={items.some((i) => i.did)}
                    count={visibleCount}
                    sort={sort}
                    setSort={setSort}
                />

                {!hasVisibleItems ? (
                    <p className={styles.empty}>No items to show.</p>
                ) : (
                    <section
                        ref={comicContainerRef}
                        className={[
                            styles.comicContainer,
                            comicsType === 'detail' ? styles.detailLayout : '',
                        ]
                            .filter(Boolean)
                            .join(' ')}
                    >
                        <ItemsGrid
                            key={uid ?? 'root'}
                            items={items}
                            bookmarks={bookmarks}
                            sort={sort}
                        />
                    </section>
                )}
            </section>
        </Layout>
    );
}
