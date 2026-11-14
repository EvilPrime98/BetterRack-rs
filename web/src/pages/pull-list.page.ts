import { UltraComponent, type UltraLightElement } from 'ultra-light-js';
import styles from './store.page.module.css';
import { StoreCard } from '../components/store-card/store-card';
import { getLatestComics } from '../services/store.service';
import { toast } from '../services/toast.service';
import type { IStorePost } from '../store.types';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { PULL_LIST_CTX } from '../context/pull-list.context';
import { matches } from '../lib/pull-list';
import { dayStart, rangeLabel, today, weekStart } from '../lib/week';

const PAGE_SIZE = 100;
const MAX_PAGES = 10;

function keyFor(item: IStorePost) {
    return item.id !== undefined ? `id:${item.id}` : `link:${item.link}`;
}

export function PullListPage() {
    const cards = new Map<string, UltraLightElement>();
    let weekPosts: IStorePost[] = [];
    let loading = false;
    let request = 0;
    const currentWeek = weekStart(today());
    let selectedWeek = currentWeek;
    let $grid: HTMLElement | null = null;
    let renderNav: () => void = () => {};

    function shownPosts(): IStorePost[] {
        const entries = PULL_LIST_CTX.items.get();
        return weekPosts.filter((p) => entries.some((e) => matches(e, p.title)));
    }

    async function loadWeek() {
        const mine = ++request;
        const after = dayStart(selectedWeek);
        const before = dayStart(selectedWeek + 7);
        weekPosts = [];
        loading = true;
        if ($grid) onGridChange($grid);
        try {
            const seen = new Set<string>();
            const all: IStorePost[] = [];
            for (let page = 1; page <= MAX_PAGES; page++) {
                const items = await getLatestComics({ page, perPage: PAGE_SIZE, after, before });
                if (mine !== request) return;
                for (const item of items) {
                    if (seen.has(keyFor(item))) continue;
                    seen.add(keyFor(item));
                    all.push(item);
                }
                if (items.length < PAGE_SIZE) break;
            }
            weekPosts = all;
        } catch (e) {
            if (mine !== request) return;
            toast.error(e instanceof Error ? e.message : 'Failed to load comics.');
        }
        loading = false;
        if ($grid) onGridChange($grid);
    }

    function selectWeek(week: number) {
        selectedWeek = week;
        renderNav();
        loadWeek();
    }

    let $empty: HTMLParagraphElement | null = null;

    function emptyText() {
        if (PULL_LIST_CTX.items.get().length === 0)
            return 'Use the pull-list button on a store card to follow a series.';
        return loading ? 'Loading…' : 'No posts match this week.';
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
        onMount: [loadWeek, () => DOCUMENT_TITLE_CONTEXT.setTitle('Pull list')],
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
                        subscriber: [PULL_LIST_CTX.items.subscribe],
                        triggerFunction: onGridChange,
                        defer: true,
                    },
                ],
            }),
        ],
    });
}
