import {
    UltraActivity,
    UltraComponent,
    ultraCompState,
    type IUltraCompStateStateful,
    type UltraLightElement,
} from 'ultra-light-js';
import styles from './recent-page.module.css';
import { Layout } from '../layout';
import { ComicCard } from '@/components/comic-card/comic-card';
import { getRecentlyAdded } from '../services/library.service';
import { toast } from '../services/toast.service';
import { BRDropdown } from '../components/br-dropdown/br-dropdown';
import { RECENT_WINDOW_OPTIONS, type ILibraryResponseItem } from '../library.types';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { LIBRARY_CONTEXT } from '../context/library.context';

interface IRecentPageState {
    items: IUltraCompStateStateful<ILibraryResponseItem[]>;
    windowHours: IUltraCompStateStateful<number>;
    isLoading: IUltraCompStateStateful<boolean>;
    error: IUltraCompStateStateful<string>;
    load: () => Promise<void>;
    dropDeleted: (uid: string) => void;
}

const WINDOW_DROPDOWN_OPTIONS = RECENT_WINDOW_OPTIONS.map((option) => ({
    value: option.label,
    label: option.label,
}));

function labelForWindow(hours: number): string {
    return (
        RECENT_WINDOW_OPTIONS.find((option) => option.hours === hours)?.label ??
        `Last ${hours} hours`
    );
}

export function RecentPage() {
    const store: IRecentPageState = ultraCompState({
        items: [] as ILibraryResponseItem[],
        windowHours: 24,
        isLoading: false,
        error: '',

        load: async (comp: IRecentPageState) => {
            comp.isLoading.set(true);
            comp.error.set('');
            try {
                const data = await getRecentlyAdded(comp.windowHours.get());
                comp.items.set(data.items);
            } catch (e) {
                const message =
                    e instanceof Error ? e.message : 'Failed to load recently added comics.';
                comp.error.set(message);
                toast.error(message);
            } finally {
                comp.isLoading.set(false);
            }
        },

        dropDeleted: (comp: IRecentPageState, uid: string) => {
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
                () =>
                    LIBRARY_CONTEXT.lastDeleted.subscribe((deleted) => {
                        if (deleted) store.dropDeleted(deleted.uid);
                    }),
                () => DOCUMENT_TITLE_CONTEXT.setTitle('Recently added'),
            ],

            children: [
                UltraComponent({
                    component: '<header></header>',
                    className: [styles.header],
                    children: [
                        `<span class="${styles.eyebrow}">Recently added</span>`,
                        UltraComponent({
                            component: `<h1 class="${styles.title}"></h1>`,
                            trigger: [
                                {
                                    subscriber: store.windowHours.subscribe,
                                    triggerFunction: ($h1: HTMLElement) => {
                                        $h1.textContent = labelForWindow(store.windowHours.get());
                                    },
                                },
                            ],
                        }),
                        BRDropdown({
                            className: [styles.filter],
                            options: WINDOW_DROPDOWN_OPTIONS,
                            value: () => labelForWindow(store.windowHours.get()),
                            subscribe: store.windowHours.subscribe,
                            onChange: (label) => {
                                const option = RECENT_WINDOW_OPTIONS.find(
                                    (opt) => opt.label === label,
                                );
                                if (!option || store.windowHours.get() === option.hours) return;
                                store.windowHours.set(option.hours);
                                store.load();
                            },
                            ariaLabel: 'Time window',
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
                                store.windowHours.subscribe,
                            ],
                            triggerFunction: ($p: HTMLElement) => {
                                $p.textContent = store.isLoading.get()
                                    ? 'Loading recently added comics…'
                                    : store.error.get()
                                      ? store.error.get()
                                      : `Nothing added in the ${labelForWindow(store.windowHours.get()).toLowerCase()}.`;
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
