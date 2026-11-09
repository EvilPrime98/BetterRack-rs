import {
    UltraActivity,
    UltraComponent,
    ultraCompState,
    type IUltraCompStateStateful,
    type UltraLightElement,
} from 'ultra-light-js';
import styles from './store.page.module.css';
import { Layout } from '../layout';
import { StoreCard } from '../components/store-card/store-card';
import { searchComics, getLatestComics } from '../services/store.service';
import { toast } from '../services/toast.service';
import type { IStorePost } from '../store.types';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { STORE_PAGE_CONTEXT } from '../context/store-page.context';
import { BRButton } from '../components/br-button/br-button';

interface IStorePageState {
    isLoading: IUltraCompStateStateful<boolean>;
    isLoadingMore: IUltraCompStateStateful<boolean>;
    error: IUltraCompStateStateful<string>;
    fetchPage: (pageNum: number) => Promise<IStorePost[]>;
    loadFirstPage: () => Promise<void>;
    loadMore: () => Promise<void>;
    runSearch: () => void;
}

export function StorePage() {
    const PAGE_SIZE = 30;

    const store: IStorePageState = ultraCompState({
        isLoading: false,
        isLoadingMore: false,
        error: '',

        fetchPage: async (_comp: IStorePageState, pageNum: number): Promise<IStorePost[]> => {
            const term = STORE_PAGE_CONTEXT.activeSearch.get();
            return term
                ? searchComics({ search: term, page: pageNum, perPage: PAGE_SIZE })
                : getLatestComics({ page: pageNum, perPage: PAGE_SIZE });
        },

        loadFirstPage: async (comp: IStorePageState) => {
            comp.isLoading.set(true);
            comp.error.set('');
            try {
                const items = await comp.fetchPage(1);
                STORE_PAGE_CONTEXT.results.set(items);
                STORE_PAGE_CONTEXT.page.set(1);
                STORE_PAGE_CONTEXT.hasMore.set(items.length === PAGE_SIZE);
                STORE_PAGE_CONTEXT.hasLoaded.set(true);
            } catch (e) {
                const message = e instanceof Error ? e.message : 'Failed to load comics.';
                comp.error.set(message);
                toast.error(message);
            } finally {
                comp.isLoading.set(false);
            }
        },

        loadMore: async (comp: IStorePageState) => {
            if (
                !STORE_PAGE_CONTEXT.hasMore.get() ||
                comp.isLoading.get() ||
                comp.isLoadingMore.get()
            )
                return;
            const nextPage = STORE_PAGE_CONTEXT.page.get() + 1;
            comp.isLoadingMore.set(true);
            try {
                const items = await comp.fetchPage(nextPage);
                STORE_PAGE_CONTEXT.results.set([...STORE_PAGE_CONTEXT.results.get(), ...items]);
                STORE_PAGE_CONTEXT.page.set(nextPage);
                STORE_PAGE_CONTEXT.hasMore.set(items.length === PAGE_SIZE);
            } catch (e) {
                const message = e instanceof Error ? e.message : 'Failed to load more comics.';
                toast.error(message);
            } finally {
                comp.isLoadingMore.set(false);
            }
        },

        runSearch: (comp: IStorePageState) => {
            STORE_PAGE_CONTEXT.activeSearch.set(STORE_PAGE_CONTEXT.query.get().trim());
            comp.loadFirstPage();
        },
    });

    function saveScrollY() {
        STORE_PAGE_CONTEXT.scrollY.set(window.scrollY);
    }

    function trackScroll() {
        const savedScrollY = STORE_PAGE_CONTEXT.scrollY.get();
        const frame = requestAnimationFrame(() => {
            if (savedScrollY > 0) window.scrollTo(0, savedScrollY);
            window.addEventListener('scroll', saveScrollY, { passive: true });
        });
        return () => {
            cancelAnimationFrame(frame);
            window.removeEventListener('scroll', saveScrollY);
        };
    }

    function loadFirstPageIfEmpty() {
        if (!STORE_PAGE_CONTEXT.hasLoaded.get()) store.loadFirstPage();
    }

    const itemsMap = new Map<string, UltraLightElement>();

    function keyFor(item: IStorePost) {
        return item.id !== undefined ? `id:${item.id}` : `link:${item.link}`;
    }

    function onQueryInput(e: Event) {
        STORE_PAGE_CONTEXT.query.set((e.target as HTMLInputElement).value);
    }

    function onQueryKeydown(e: Event) {
        if ((e as KeyboardEvent).key === 'Enter') store.runSearch();
    }

    function onErrorChange($p: HTMLElement) {
        $p.textContent = store.error.get();
    }

    function onSentinelMount($sentinel: HTMLElement) {
        const observer = new IntersectionObserver(
            (entries) => {
                if (!entries.some((e) => e.isIntersecting)) return;
                store.loadMore();
            },
            { rootMargin: '600px' },
        );
        observer.observe($sentinel);
        return () => observer.disconnect();
    }

    const sentinel = UltraComponent({
        component: `<div class="${styles.sentinel}"></div>`,
        onMount: [onSentinelMount],
    });

    const loadingMore = UltraComponent({
        component: `<p class="${styles.loadingMore}">Loading more…</p>`,
    });

    function onResultsChange($section: HTMLElement) {
        const currResults = STORE_PAGE_CONTEXT.results.get();

        if (currResults.length === 0) {
            for (const node of itemsMap.values()) {
                node._cleanup?.();
                node.remove();
            }

            itemsMap.clear();
        } else {
            const currKeys = new Set(currResults.map(keyFor));

            for (const [key, node] of itemsMap) {
                if (currKeys.has(key)) continue;
                node._cleanup?.();
                node.remove();
                itemsMap.delete(key);
            }

            currResults.forEach((item) => {
                const key = keyFor(item);
                if (itemsMap.has(key)) return;
                itemsMap.set(key, StoreCard({ item }));
            });

            currResults.forEach((item) => {
                const node = itemsMap.get(keyFor(item));
                if (node) $section.appendChild(node);
            });

            $section.appendChild(sentinel);

            if (store.isLoadingMore.get()) $section.appendChild(loadingMore);
            else loadingMore.remove();
        }
    }

    return Layout(
        UltraComponent({
            component: '<section></section>',

            className: [styles.page],

            onMount: [
                loadFirstPageIfEmpty,
                () => DOCUMENT_TITLE_CONTEXT.setTitle('Store'),
                trackScroll,
            ],

            children: [
                `<h1 class="${styles.title}">Store</h1>`,

                UltraComponent({
                    component: '<div></div>',

                    className: [styles.searchRow],

                    children: [
                        UltraComponent({
                            component: '<input type="text" />',
                            className: [styles.searchInput],
                            attributes: {
                                placeholder: 'Search for a comic…',
                                'aria-label': 'Search comics',
                            },
                            trigger: [
                                {
                                    subscriber: STORE_PAGE_CONTEXT.query.subscribe,
                                    triggerFunction: ($el: HTMLElement) => {
                                        ($el as HTMLInputElement).value =
                                            STORE_PAGE_CONTEXT.query.get();
                                    },
                                },
                            ],
                            eventHandler: {
                                input: onQueryInput,
                                keydown: onQueryKeydown,
                            },
                        }),

                        BRButton({
                            text: 'Search',
                            eventHandler: { click: store.runSearch },
                        }),
                    ],
                }),

                UltraComponent({
                    component: `<p class="${styles.errorText}"></p>`,
                    trigger: [
                        {
                            subscriber: store.error.subscribe,
                            triggerFunction: onErrorChange,
                        },
                    ],
                }),

                UltraActivity({
                    mode: {
                        subscriber: [
                            store.isLoading.subscribe,
                            STORE_PAGE_CONTEXT.results.subscribe,
                        ],
                        state: () =>
                            !store.isLoading.get() && STORE_PAGE_CONTEXT.results.get().length === 0,
                    },
                    component: '<p></p>',
                    className: [styles.empty],
                    trigger: [
                        {
                            subscriber: [
                                STORE_PAGE_CONTEXT.results.subscribe,
                                store.isLoading.subscribe,
                                store.isLoadingMore.subscribe,
                                STORE_PAGE_CONTEXT.hasMore.subscribe,
                            ],
                            triggerFunction: ($p: HTMLElement) => {
                                $p.textContent = store.isLoading.get()
                                    ? 'Loading…'
                                    : 'No comics found.';
                            },
                        },
                    ],
                }),

                UltraComponent({
                    component: '<section></section>',
                    className: [styles.grid],
                    onMount: [onResultsChange],
                    trigger: [
                        {
                            subscriber: [
                                STORE_PAGE_CONTEXT.results.subscribe,
                                store.isLoading.subscribe,
                                store.isLoadingMore.subscribe,
                                STORE_PAGE_CONTEXT.hasMore.subscribe,
                            ],
                            triggerFunction: onResultsChange,
                            defer: true,
                        },
                    ],
                }),
            ],
        }),
    );
}
