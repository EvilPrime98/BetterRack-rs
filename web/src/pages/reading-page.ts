import {
    UltraActivity,
    UltraComponent,
    ultraCompState,
    type IUltraCompStateStateful,
    type UltraLightElement,
} from 'ultra-light-js';
import styles from './reading-page.module.css';
import { Layout } from '../layout';
import { ComicCard } from '@/components/comic-card/comic-card';
import { LayoutSelector } from '@/components/layout/layout-selector';
import { getReading } from '../services/library.service';
import { toast } from '../services/toast.service';
import type { ILibraryResponseItem } from '../library.types';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { COMIC_CACHE_CONTEXT } from '../context/comic-cache.context';
import { LIBRARY_CONTEXT } from '../context/library.context';
import { matchesReadFilter } from '../context/read-types.context';

interface IReadingPageState {
    items: IUltraCompStateStateful<ILibraryResponseItem[]>;
    isLoading: IUltraCompStateStateful<boolean>;
    error: IUltraCompStateStateful<string>;
    load: () => Promise<void>;
    dropNotInProgress: () => void;
    dropDeleted: (uid: string) => void;
}

export function ReadingPage() {
    const store: IReadingPageState = ultraCompState({
        items: [] as ILibraryResponseItem[],
        isLoading: false,
        error: '',

        load: async (comp: IReadingPageState) => {
            comp.isLoading.set(true);
            comp.error.set('');
            try {
                await COMIC_CACHE_CONTEXT.flushPending();
                const data = await getReading();
                comp.items.set(data.items);
            } catch (e) {
                const message =
                    e instanceof Error ? e.message : 'Failed to load comics in progress.';
                comp.error.set(message);
                toast.error(message);
            } finally {
                comp.isLoading.set(false);
            }
        },

        dropNotInProgress: (comp: IReadingPageState) => {
            const currItems = comp.items.get();
            const inProgress = currItems.filter((item) =>
                matchesReadFilter(
                    'reading',
                    COMIC_CACHE_CONTEXT.getCacheById(item.uid)?.readPer ?? 0,
                ),
            );
            if (inProgress.length !== currItems.length) comp.items.set(inProgress);
        },

        dropDeleted: (comp: IReadingPageState, uid: string) => {
            const currItems = comp.items.get();
            const remaining = currItems.filter((item) => item.uid !== uid);
            if (remaining.length !== currItems.length) comp.items.set(remaining);
        },
    });

    const itemsMap = new Map<string, UltraLightElement>();

    function onItemsChange($section: HTMLElement) {
        const currItems = store.items.get();
        const currIds = new Set(currItems.map((item) => item.uid));

        for (const [uid, node] of itemsMap) {
            if (currIds.has(uid)) continue;
            node._cleanup?.();
            node.remove();
            itemsMap.delete(uid);
        }

        currItems.forEach((item) => {
            if (itemsMap.has(item.uid)) return;
            itemsMap.set(item.uid, ComicCard({ item }));
        });

        currItems.forEach((item) => {
            const node = itemsMap.get(item.uid);
            if (node) $section.appendChild(node);
        });
    }

    return Layout(
        UltraComponent({
            component: '<section></section>',

            className: [styles.page],

            onMount: [
                () => {
                    store.load();
                },
                () => COMIC_CACHE_CONTEXT.cache.subscribe(() => store.dropNotInProgress()),
                () =>
                    LIBRARY_CONTEXT.lastDeleted.subscribe((deleted) => {
                        if (deleted) store.dropDeleted(deleted.uid);
                    }),
                () => DOCUMENT_TITLE_CONTEXT.setTitle('Keep reading'),
            ],

            children: [
                UltraComponent({
                    component: '<header></header>',
                    className: [styles.header],
                    children: [
                        UltraComponent({
                            component: '<div></div>',
                            className: [styles.left],
                            children: [
                                `<span class="${styles.eyebrow}">Keep reading</span>`,
                                `<h1 class="${styles.title}">Currently reading</h1>`,
                            ],
                        }),

                        UltraComponent({
                            component: '<div></div>',
                            className: [styles.right],
                            children: [LayoutSelector()],
                        }),
                    ],
                }),

                UltraActivity({
                    mode: {
                        subscriber: [
                            store.isLoading.subscribe,
                            store.items.subscribe,
                            store.error.subscribe,
                        ],
                        state: () =>
                            store.isLoading.get() ||
                            !!store.error.get() ||
                            store.items.get().length === 0,
                    },
                    component: '<p></p>',
                    className: [styles.empty],
                    trigger: [
                        {
                            subscriber: [
                                store.isLoading.subscribe,
                                store.items.subscribe,
                                store.error.subscribe,
                            ],
                            triggerFunction: ($p: HTMLElement) => {
                                $p.textContent = store.isLoading.get()
                                    ? 'Loading comics in progress…'
                                    : store.error.get()
                                      ? store.error.get()
                                      : 'Nothing in progress.';
                            },
                        },
                    ],
                }),

                UltraComponent({
                    component: '<section></section>',
                    className: [styles.comicContainer],
                    onMount: [onItemsChange],
                    trigger: [
                        {
                            subscriber: store.items.subscribe,
                            triggerFunction: onItemsChange,
                            defer: true,
                        },
                    ],
                }),
            ],
        }),
    );
}
