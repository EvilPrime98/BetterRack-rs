import { UltraComponent, type UltraLightElement } from 'ultra-light-js';
import styles from './store.page.module.css';
import { Layout } from '../layout';
import { StoreCard } from '../components/store-card/store-card';
import { getLatestComics } from '../services/store.service';
import { toast } from '../services/toast.service';
import type { IStorePost } from '../store.types';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { PULL_LIST_CTX } from '../context/pull-list.context';
import { STORE_PAGE_CONTEXT } from '../context/store-page.context';
import { matches } from '../lib/pull-list';

const PAGE_SIZE = 30;
/** Matches wanted before the page stops fetching on its own. */
const FILL_TARGET = 24;
/** Bound on the automatic fetching, so a series that never matches cannot page through the site. */
const MAX_PAGES = 30;

function keyFor(item: IStorePost) {
    return item.id !== undefined ? `id:${item.id}` : `link:${item.link}`;
}

export function PullListPage() {
    const cards = new Map<string, UltraLightElement>();
    let loading = false;

    function shownPosts(): IStorePost[] {
        const entries = PULL_LIST_CTX.items.get();
        return STORE_PAGE_CONTEXT.results
            .get()
            .filter((p) => entries.some((e) => matches(e, p.title)));
    }

    async function loadFirstPage() {
        loading = true;
        try {
            const items = await getLatestComics({ page: 1, perPage: PAGE_SIZE });
            STORE_PAGE_CONTEXT.results.set(items);
            STORE_PAGE_CONTEXT.page.set(1);
            STORE_PAGE_CONTEXT.hasMore.set(items.length === PAGE_SIZE);
            STORE_PAGE_CONTEXT.hasLoaded.set(true);
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to load comics.');
        } finally {
            loading = false;
            fillIfSparse();
        }
    }

    async function loadMore() {
        if (loading || !STORE_PAGE_CONTEXT.hasMore.get()) return;
        const nextPage = STORE_PAGE_CONTEXT.page.get() + 1;
        loading = true;
        try {
            const items = await getLatestComics({ page: nextPage, perPage: PAGE_SIZE });
            STORE_PAGE_CONTEXT.results.set([...STORE_PAGE_CONTEXT.results.get(), ...items]);
            STORE_PAGE_CONTEXT.page.set(nextPage);
            STORE_PAGE_CONTEXT.hasMore.set(items.length === PAGE_SIZE);
        } catch (e) {
            STORE_PAGE_CONTEXT.hasMore.set(false);
            toast.error(e instanceof Error ? e.message : 'Failed to load more comics.');
        } finally {
            loading = false;
            fillIfSparse();
        }
    }

    /** Too few matches to scroll: keep fetching until there are enough or the bound is hit. */
    function fillIfSparse() {
        if (
            PULL_LIST_CTX.items.get().length > 0 &&
            shownPosts().length < FILL_TARGET &&
            STORE_PAGE_CONTEXT.page.get() < MAX_PAGES
        )
            loadMore();
    }

    function start() {
        // The feed is shared with the Store page: a leftover search would hide the latest posts.
        if (STORE_PAGE_CONTEXT.activeSearch.get()) {
            STORE_PAGE_CONTEXT.activeSearch.set('');
            STORE_PAGE_CONTEXT.query.set('');
            loadFirstPage();
        } else if (!STORE_PAGE_CONTEXT.hasLoaded.get()) loadFirstPage();
        else fillIfSparse();
    }

    function onSentinelMount($sentinel: HTMLElement) {
        const observer = new IntersectionObserver(
            (entries) => {
                // A sparse list is filled (and bounded) by fillIfSparse, not by the sentinel.
                if (entries.some((e) => e.isIntersecting) && shownPosts().length >= FILL_TARGET)
                    loadMore();
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

    let shownKeys: string[] = [];
    let $empty: HTMLParagraphElement | null = null;

    function emptyText() {
        if (PULL_LIST_CTX.items.get().length === 0)
            return 'Use the pull-list button on a store card to follow a series.';
        return STORE_PAGE_CONTEXT.hasMore.get() ? 'Loading…' : 'No posts match your pull list yet.';
    }

    function onGridChange($section: HTMLElement) {
        const posts = shownPosts();
        const keys = posts.map(keyFor);
        const grewOnly =
            shownKeys.length > 0 && shownKeys.every((key, i) => key === keys[i]) && $empty === null;

        if (grewOnly) {
            // Same posts plus newer ones: add only those, so the cards on screen are untouched.
            for (const post of posts.slice(shownKeys.length)) {
                const node = StoreCard({ item: post });
                cards.set(keyFor(post), node);
                $section.insertBefore(node, sentinel);
            }
        } else {
            const wanted = new Set(keys);
            for (const [key, node] of cards) {
                if (wanted.has(key)) continue;
                node._cleanup?.();
                cards.delete(key);
            }
            for (const post of posts) {
                if (!cards.has(keyFor(post))) cards.set(keyFor(post), StoreCard({ item: post }));
            }
            $section.replaceChildren(...keys.map((key) => cards.get(key) as Node));
            $empty = null;
            if (posts.length === 0) {
                $empty = document.createElement('p');
                $empty.className = styles.empty;
                $section.appendChild($empty);
            }
            $section.appendChild(sentinel);
        }
        if ($empty) $empty.textContent = emptyText();
        shownKeys = keys;
        fillIfSparse();
    }

    return Layout(
        UltraComponent({
            component: '<section></section>',
            className: [styles.page],
            onMount: [start, () => DOCUMENT_TITLE_CONTEXT.setTitle('Pull list')],
            children: [
                `<h1 class="${styles.title}">Pull list</h1>`,
                UltraComponent({
                    component: '<section></section>',
                    className: [styles.grid],
                    onMount: [onGridChange],
                    trigger: [
                        {
                            subscriber: [
                                STORE_PAGE_CONTEXT.results.subscribe,
                                STORE_PAGE_CONTEXT.hasMore.subscribe,
                                PULL_LIST_CTX.items.subscribe,
                            ],
                            triggerFunction: onGridChange,
                            defer: true,
                        },
                    ],
                }),
            ],
        }),
    );
}
