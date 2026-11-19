import {
    UltraActivity,
    UltraComponent,
    ultraQueryParams,
    ultraState,
    type UltraLightElement,
} from 'ultra-light-js';
import styles from './library-page.module.css';
import { PageHeader } from '@/components/page-header/page-header';
import { LIBRARY_CONTEXT } from '../context/library.context';
import { FolderCard } from '@/components/folder-card/folder-card';
import { ComicCard } from '@/components/comic-card/comic-card';
import { COMICS_TYPE_CTX } from '../context/comics-types.context';
import type { ILibraryResponseItem } from '../library.types';
import { ultraFilters } from '../hooks/ultraFilters';
import { SearchPage } from './search.page';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { matchesReadFilter, READ_TYPES_CTX } from '../context/read-types.context';
import { COMIC_CACHE_CONTEXT } from '../context/comic-cache.context';
import { BOOKMARKS_CTX } from '../context/bookmarks.context';
import { BookmarkCard } from '@/components/bookmark-card/bookmark-card';
import { bookmarksInFolder, cellKey, mergeBookmarks, type TGridCell } from '../lib/bookmarks';

const PAGE_SIZE = 60;

export function LibraryPage({ uid }: { uid?: string }) {
    const { search } = ultraQueryParams();
    if (search) {
        return SearchPage({ search });
    }

    const itemsMap = new Map<string, UltraLightElement>();

    let renderCount = PAGE_SIZE;
    let sentinelObserver: IntersectionObserver | null = null;
    const $sentinel = document.createElement('div');
    $sentinel.style.gridColumn = '1 / -1';
    $sentinel.style.height = '1px';

    const [items, setItems, subsItems] = ultraState<ILibraryResponseItem[]>([]);

    const { filters, resetFilters, applyFilters } = ultraFilters({
        rawItems: getLibraryItems,
        setItems,
    });

    function onLayoutChange($section: HTMLElement) {
        $section.classList.toggle(styles.detailLayout, COMICS_TYPE_CTX.type.get() === 'detail');
    }

    function visibleBookmarks() {
        if (LIBRARY_CONTEXT.structure.get() !== 'folders') return [];
        const query = LIBRARY_CONTEXT.searchQuery.get().trim().toLowerCase();
        const readFilter = READ_TYPES_CTX.type.get();
        return bookmarksInFolder(BOOKMARKS_CTX.items.get(), uid).filter(
            (b) =>
                matchesReadFilter(readFilter, b.read ? 100 : 0) &&
                (!query || b.title.toLowerCase().includes(query)),
        );
    }

    function onItemsChange($section: HTMLElement) {
        const allComics: TGridCell[] = mergeBookmarks(
            [...items()],
            visibleBookmarks(),
            filters.sortByReleaseDate.get(),
        );
        const currComics = allComics.slice(0, renderCount);
        const currIds = new Set(currComics.map(cellKey));

        for (const [uid, node] of itemsMap) {
            if (currIds.has(uid)) continue;
            node._cleanup?.();
            node.remove();
            itemsMap.delete(uid);
        }

        currComics.map((cell) => {
            const key = cellKey(cell);
            if (itemsMap.has(key)) return;
            if (cell.type === 'bookmark') {
                itemsMap.set(key, BookmarkCard({ bookmark: cell.bookmark }));
                return;
            }
            const item = cell.item;
            itemsMap.set(
                key,
                item.did
                    ? FolderCard({
                          title: item.name,
                          uid: item.uid,
                      })
                    : ComicCard({
                          item: item,
                      }),
            );
        });

        currComics.map((c) => {
            const $item = itemsMap.get(cellKey(c));
            if ($item) $section.appendChild($item);
        });

        if (renderCount < allComics.length) {
            $section.appendChild($sentinel);
        } else {
            $sentinel.remove();
        }
    }

    function getLibraryItems() {
        const items = LIBRARY_CONTEXT.getLibraryItems({ onlyDir: !uid, uid });
        if (!uid && LIBRARY_CONTEXT.structure.get() === 'folders') {
            const rootComics = LIBRARY_CONTEXT.groups
                .get()
                .flatMap((g) => g.entries)
                .filter((e) => !e.did && !e.parentId);
            items.push(...rootComics);
        }
        const query = LIBRARY_CONTEXT.searchQuery.get().trim().toLowerCase();
        if (!query) return items;
        return items.filter((item) => item.name.toLowerCase().includes(query));
    }

    function hasVisibleItems() {
        const readFilter = READ_TYPES_CTX.type.get();
        if (visibleBookmarks().length) return true;
        return items().some(
            (item) =>
                item.did ||
                matchesReadFilter(
                    readFilter,
                    COMIC_CACHE_CONTEXT.getCacheById(item.uid)?.readPer || 0,
                ),
        );
    }

    return UltraComponent({
        component: '<section></section>',

        className: [styles.page],

        onMount: [() => DOCUMENT_TITLE_CONTEXT.setTitle('Library')],

        children: [
            PageHeader({
                uid,
                items,
                subsItems,
                filters,
                resetFilters,
                showNewFolder: true,
            }),

            UltraActivity({
                mode: {
                    subscriber: [
                        subsItems,
                        LIBRARY_CONTEXT.libraryLoaded.subscribe,
                        READ_TYPES_CTX.type.subscribe,
                        COMIC_CACHE_CONTEXT.cache.subscribe,
                        BOOKMARKS_CTX.items.subscribe,
                    ],
                    state: () => LIBRARY_CONTEXT.libraryLoaded.get() && !hasVisibleItems(),
                },
                component: `<p class="${styles.empty}">No items to show.</p>`,
            }),

            UltraComponent({
                onMount: [
                    ($el: HTMLElement) => {
                        LIBRARY_CONTEXT.fetchLibrary();
                        if (LIBRARY_CONTEXT.groups.get().length) applyFilters();
                        $el.scrollTo(0, 0);

                        sentinelObserver = new IntersectionObserver(
                            (entries) => {
                                if (!entries.some((e) => e.isIntersecting)) return;
                                renderCount += PAGE_SIZE;
                                sentinelObserver?.unobserve($sentinel);
                                onItemsChange($el);
                                if ($sentinel.isConnected) sentinelObserver?.observe($sentinel);
                            },
                            { rootMargin: '600px' },
                        );
                        sentinelObserver.observe($sentinel);

                        return () => sentinelObserver?.disconnect();
                    },
                ],

                component: '<section></section>',
                className: [styles.comicContainer],
                trigger: [
                    {
                        subscriber: subsItems,
                        triggerFunction: onItemsChange,
                        defer: true,
                    },
                    {
                        subscriber: [COMICS_TYPE_CTX.type.subscribe, subsItems],
                        triggerFunction: onLayoutChange,
                        defer: true,
                    },
                    {
                        subscriber: [
                            BOOKMARKS_CTX.items.subscribe,
                            READ_TYPES_CTX.type.subscribe,
                            filters.sortByReleaseDate.subscribe,
                        ],
                        triggerFunction: onItemsChange,
                        defer: true,
                    },
                ],
            }),
        ],

        trigger: [
            {
                subscriber: [
                    LIBRARY_CONTEXT.groups.subscribe,
                    LIBRARY_CONTEXT.searchQuery.subscribe,
                ],
                triggerFunction: () => applyFilters(),
            },
        ],
    });
}
