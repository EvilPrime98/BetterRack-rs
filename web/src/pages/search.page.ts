import { UltraActivity, UltraComponent, ultraState, type UltraLightElement } from 'ultra-light-js';
import styles from './search-page.module.css';
import { PageHeader } from '../components/page-header/page-header';
import { LIBRARY_CONTEXT } from '../context/library.context';
import { ComicCard } from '../components/comic-card/comic-card';
import { COMICS_TYPE_CTX } from '../context/comics-types.context';
import type { ILibraryResponseItem } from '../library.types';
import { ultraFilters } from '../hooks/ultraFilters';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { matchesReadFilter, READ_TYPES_CTX } from '../context/read-types.context';
import { filterByWriter } from '../lib/writer-filter';
import { COMIC_CACHE_CONTEXT } from '../context/comic-cache.context';

export function SearchPage({ search, writer }: { search: string; writer?: string }) {
    const PAGE_SIZE = 60;

    if (writer === undefined && LIBRARY_CONTEXT.searchQuery.get() !== search) {
        LIBRARY_CONTEXT.searchQuery.set(search);
    }

    const [items, setItems, subsItems] = ultraState<ILibraryResponseItem[]>([]);

    const [visibleCount, setVisibleCount, subsVisibleCount] = ultraState(PAGE_SIZE);

    const [libLoading, setLibLoading, subsLibLoading] = ultraState(
        !LIBRARY_CONTEXT.libraryLoaded.get(),
    );
    LIBRARY_CONTEXT.libraryLoaded.subscribe((loaded) => setLibLoading(!loaded));

    const { filters, resetFilters, applyFilters } = ultraFilters({
        rawItems: getSearchItems,
        setItems,
    });

    const itemsMap = new Map<string, UltraLightElement>();

    const sentinel = UltraComponent({
        component: `<div class="${styles.sentinel}"></div>`,
        onMount: [onSentinelMount],
    });

    function hasMore() {
        return visibleCount() < items().length;
    }

    function loadMore() {
        if (!hasMore()) return;
        setVisibleCount(Math.min(visibleCount() + PAGE_SIZE, items().length));
    }

    function onSentinelMount($sentinel: HTMLElement) {
        const observer = new IntersectionObserver(
            (entries) => {
                if (!entries.some((e) => e.isIntersecting)) return;
                if (!hasMore()) {
                    observer.disconnect();
                    return;
                }
                loadMore();
            },
            { rootMargin: '600px' },
        );
        observer.observe($sentinel);
        return () => observer.disconnect();
    }

    function onLayoutChange($section: HTMLElement) {
        $section.classList.toggle(styles.detailLayout, COMICS_TYPE_CTX.type.get() === 'detail');
    }

    function onItemsChange($section: HTMLElement) {
        const currComics = items().slice(0, visibleCount());
        const currIds = new Set(currComics.map((c) => c.uid));

        for (const [uid, node] of itemsMap) {
            if (currIds.has(uid)) continue;
            node._cleanup?.();
            node.remove();
            itemsMap.delete(uid);
        }

        currComics.map((item) => {
            if (itemsMap.has(item.uid)) return;
            itemsMap.set(item.uid, ComicCard({ item }));
        });

        currComics.map((c) => {
            const $item = itemsMap.get(c.uid);
            if ($item) $section.appendChild($item);
        });

        $section.appendChild(sentinel);
    }

    function getSearchItems() {
        if (writer !== undefined) {
            return filterByWriter(LIBRARY_CONTEXT.getLibraryItems({ onlyDir: false }), writer);
        }
        const query = LIBRARY_CONTEXT.searchQuery.get().trim().toLowerCase();
        return LIBRARY_CONTEXT.getLibraryItems({ onlyDir: false })
            .filter((item) => item.did === false)
            .filter((item) => item.name.toLowerCase().includes(query));
    }

    function hasVisibleItems() {
        const readFilter = READ_TYPES_CTX.type.get();
        return items().some((item) =>
            matchesReadFilter(readFilter, COMIC_CACHE_CONTEXT.getCacheById(item.uid)?.readPer || 0),
        );
    }

    subsItems(() => setVisibleCount(PAGE_SIZE));

    return UltraComponent({
        component: '<section></section>',

        className: [styles.page],

        onMount: [
            () =>
                DOCUMENT_TITLE_CONTEXT.setTitle(
                    writer === undefined ? `Search: "${search}"` : 'Filtered',
                ),
        ],

        children: [
            ...(writer === undefined
                ? []
                : [
                      `<p class="${styles.loadingNote}">Filtered by writer: <strong></strong> · <a href="/">Clear</a></p>`,
                  ]),
            PageHeader({
                items,
                subsItems,
                filters,
                resetFilters,
            }),

            UltraActivity({
                mode: { state: libLoading, subscriber: subsLibLoading },
                component: '<p></p>',
                className: [styles.loadingNote],
                children: ['Still loading your library — search results may be incomplete.'],
            }),

            UltraActivity({
                mode: {
                    subscriber: [
                        subsItems,
                        subsLibLoading,
                        READ_TYPES_CTX.type.subscribe,
                        COMIC_CACHE_CONTEXT.cache.subscribe,
                    ],
                    state: () => !libLoading() && !hasVisibleItems(),
                },
                component: `<p class="${styles.empty}">No items to show.</p>`,
            }),

            UltraComponent({
                onMount: [
                    () => {
                        LIBRARY_CONTEXT.fetchLibrary();
                        if (LIBRARY_CONTEXT.groups.get().length) applyFilters();
                    },
                ],
                component: '<section></section>',
                className: [styles.comicContainer],
                trigger: [
                    {
                        subscriber: [subsItems, subsVisibleCount],
                        triggerFunction: onItemsChange,
                        defer: true,
                    },
                    {
                        subscriber: [COMICS_TYPE_CTX.type.subscribe, subsItems],
                        triggerFunction: onLayoutChange,
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
