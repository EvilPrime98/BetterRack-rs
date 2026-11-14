import { UltraComponent, type UltraLightElement } from 'ultra-light-js';
import styles from './store.page.module.css';
import { StoreCard } from '../components/store-card/store-card';
import { getLatestComics } from '../services/store.service';
import { toast } from '../services/toast.service';
import type { IStorePost } from '../store.types';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { PULL_LIST_CTX } from '../context/pull-list.context';
import { STORE_PAGE_CONTEXT } from '../context/store-page.context';
import { matches } from '../lib/pull-list';
import { inWeek, parseDay, rangeLabel, today, weekStart } from '../lib/week';

const PAGE_SIZE = 30;
const MAX_PAGES = 30;

function keyFor(item: IStorePost) {
    return item.id !== undefined ? `id:${item.id}` : `link:${item.link}`;
}

export function PullListPage() {
    const cards = new Map<string, UltraLightElement>();
    let loading = false;
    const currentWeek = weekStart(today());
    let selectedWeek = currentWeek;
    let $grid: HTMLElement | null = null;
    let renderNav: () => void = () => {};

    function postDay(p: IStorePost) {
        return p.uploadDate ? parseDay(p.uploadDate) : undefined;
    }

    function shownPosts(): IStorePost[] {
        const entries = PULL_LIST_CTX.items.get();
        return STORE_PAGE_CONTEXT.results.get().filter((p) => {
            const day = postDay(p);
            return (
                day !== undefined &&
                inWeek(day, selectedWeek) &&
                entries.some((e) => matches(e, p.title))
            );
        });
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

    function fillIfSparse() {
        if (PULL_LIST_CTX.items.get().length === 0) return;
        const days = STORE_PAGE_CONTEXT.results
            .get()
            .map(postDay)
            .filter((d): d is number => d !== undefined);
        const reachedBack = days.length > 0 && Math.min(...days) < selectedWeek;
        if (!reachedBack && STORE_PAGE_CONTEXT.page.get() < MAX_PAGES) loadMore();
    }

    function selectWeek(week: number) {
        selectedWeek = week;
        renderNav();
        if ($grid) onGridChange($grid);
    }

    function start() {
        if (STORE_PAGE_CONTEXT.activeSearch.get()) {
            STORE_PAGE_CONTEXT.activeSearch.set('');
            STORE_PAGE_CONTEXT.query.set('');
            loadFirstPage();
        } else if (!STORE_PAGE_CONTEXT.hasLoaded.get()) loadFirstPage();
        else fillIfSparse();
    }

    let $empty: HTMLParagraphElement | null = null;

    function emptyText() {
        if (PULL_LIST_CTX.items.get().length === 0)
            return 'Use the pull-list button on a store card to follow a series.';
        return STORE_PAGE_CONTEXT.hasMore.get() ? 'Loading…' : 'No posts match this week.';
    }

    function onGridChange($section: HTMLElement) {
        $grid = $section;
        const posts = shownPosts();
        const wanted = new Set(posts.map(keyFor));
        for (const [key, node] of cards) {
            if (wanted.has(key)) continue;
            node._cleanup?.();
            cards.delete(key);
        }
        for (const post of posts) {
            if (!cards.has(keyFor(post))) cards.set(keyFor(post), StoreCard({ item: post }));
        }
        $section.replaceChildren(...posts.map((post) => cards.get(keyFor(post)) as Node));
        $empty = null;
        if (posts.length === 0) {
            $empty = document.createElement('p');
            $empty.className = styles.empty;
            $empty.textContent = emptyText();
            $section.appendChild($empty);
        }
        fillIfSparse();
    }

    function onNavMount($nav: HTMLElement) {
        const button = (label: string, aria: string, onClick: () => void) => {
            const $b = document.createElement('button');
            $b.type = 'button';
            $b.className = styles.weekBtn;
            $b.textContent = label;
            $b.setAttribute('aria-label', aria);
            $b.addEventListener('click', onClick);
            return $b;
        };
        const $prev = button('‹', 'Previous week', () => selectWeek(selectedWeek - 7));
        const $label = document.createElement('span');
        $label.className = styles.weekLabel;
        const $next = button('›', 'Next week', () => selectWeek(selectedWeek + 7));
        const $now = button('This week', 'Jump to this week', () => selectWeek(currentWeek));
        renderNav = () => {
            $label.textContent = rangeLabel(selectedWeek);
            $next.disabled = $now.disabled = selectedWeek >= currentWeek;
        };
        $nav.replaceChildren($prev, $label, $next, $now);
        renderNav();
    }

    return UltraComponent({
        component: '<section></section>',
        className: [styles.page],
        onMount: [start, () => DOCUMENT_TITLE_CONTEXT.setTitle('Pull list')],
        children: [
            `<h1 class="${styles.title}">Pull list</h1>`,
            UltraComponent({
                component: '<div></div>',
                className: [styles.weekNav],
                onMount: [onNavMount],
            }),
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
    });
}
