import { UltraComponent } from 'ultra-light-js';
import { ArrowLeftIcon } from '../../icons/arrow-left.icon';
import { RefreshIcon } from '../../icons/refresh-icon';
import type { IBookmark } from '../../library.types';
import type { TReadingMode } from '../../context/user-pref-cache.context';
import { WindowControls } from '../window-controls/window-controls';
import styles from '../../pages/reader.page.module.css';

export function ReaderPageHeader({
    title,
    subsTitle,
    currentPage,
    subsCurrentPage,
    pages,
    subsPages,
    bookmarks,
    subsBookmarks,
    isRefreshing,
    subsIsRefreshing,
    readingMode,
    onReadingModeChange,
    goToPage,
    goBack,
    onRefresh,
}: {
    title: () => string;
    subsTitle: (fn: (value: string) => void) => () => void;
    currentPage: () => number;
    subsCurrentPage: (fn: (value: number) => void) => () => void;
    pages: () => string[];
    subsPages: (fn: (value: string[]) => void) => () => void;
    bookmarks: () => IBookmark[];
    subsBookmarks: (fn: (value: IBookmark[]) => void) => () => void;
    isRefreshing: () => boolean;
    subsIsRefreshing: (fn: (value: boolean) => void) => () => void;
    readingMode: () => TReadingMode;
    onReadingModeChange: (mode: TReadingMode) => void;
    goToPage: (page: number) => void;
    goBack: () => void;
    onRefresh: () => void;
}) {
    const onRefreshingChange = ($button: HTMLElement) => {
        $button.classList.toggle(styles.spinning, isRefreshing());
    };

    const onTitleChange = ($el: HTMLElement) => {
        $el.textContent = title();
        $el.title = title();
    };

    const onCounterInputChange = ($el: HTMLElement) => {
        const $input = $el as HTMLInputElement;
        $input.value = String(currentPage());
        $input.style.width = `${String(pages().length).length + 3}ch`;
    };

    const onCounterTotalChange = ($el: HTMLElement) => {
        $el.textContent = ` / ${pages().length}`;
    };

    const onCounterKeyDown = (evt: Event) => {
        const $input = evt.target as HTMLInputElement;
        const key = (evt as KeyboardEvent).key;
        if (key === 'Enter') {
            $input.blur();
        } else if (key === 'Escape') {
            $input.value = String(currentPage());
            $input.blur();
        }
    };

    const onCounterBlur = (evt: Event) => {
        const $input = evt.target as HTMLInputElement;
        const page = Number($input.value.trim());
        if (Number.isInteger(page) && page >= 1) {
            const target = Math.min(page, pages().length);
            if (target !== currentPage()) goToPage(target);
        }
        $input.value = String(currentPage());
    };

    const renderBookmarks = ($el: HTMLElement) => {
        const $select = $el as HTMLSelectElement;
        const items = bookmarks();

        $select.replaceChildren();
        $select.hidden = items.length === 0;
        if (!items.length) return;

        const $placeholder = document.createElement('option');
        $placeholder.value = '';
        $placeholder.textContent = 'Jump to bookmark…';
        $select.appendChild($placeholder);

        for (const bookmark of items) {
            const $option = document.createElement('option');
            $option.value = String(bookmark.page);
            $option.textContent = `${bookmark.label} · p.${bookmark.page}`;
            $select.appendChild($option);
        }
    };

    const onBookmarkSelect = (evt: Event) => {
        const $select = evt.target as HTMLSelectElement;
        const page = Number($select.value);
        if (Number.isInteger(page) && page >= 1) goToPage(page);
        $select.value = '';
    };

    const onReadingModeMount = ($el: HTMLElement) => {
        ($el as HTMLSelectElement).value = readingMode();
    };

    const onReadingModeSelect = (evt: Event) => {
        onReadingModeChange((evt.target as HTMLSelectElement).value as TReadingMode);
    };

    return UltraComponent({
        component: '<header></header>',
        className: [styles.toolbar],
        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.toolbarStart],
                children: [
                    UltraComponent({
                        component: '<button type="button"></button>',
                        className: [styles.back],
                        eventHandler: { click: goBack },
                        children: [ArrowLeftIcon({ size: 16 }), '<span>Library</span>'],
                    }),
                    UltraComponent({
                        component: '<select hidden></select>',
                        className: [styles.bookmarks],
                        eventHandler: { change: onBookmarkSelect },
                        onMount: [renderBookmarks],
                        trigger: [{ subscriber: subsBookmarks, triggerFunction: renderBookmarks }],
                    }),
                ],
            }),
            UltraComponent({
                component: '<h1></h1>',
                className: [styles.title],
                onMount: [onTitleChange],
                trigger: [{ subscriber: subsTitle, triggerFunction: onTitleChange }],
            }),
            UltraComponent({
                component: '<div></div>',
                className: [styles.toolbarEnd],
                children: [
                    UltraComponent({
                        component:
                            '<select><option value="vertical">Vertical</option><option value="horizontal">Horizontal</option></select>',
                        className: [styles.bookmarks],
                        attributes: { 'aria-label': 'Reading mode' },
                        eventHandler: { change: onReadingModeSelect },
                        onMount: [onReadingModeMount],
                    }),
                    UltraComponent({
                        component: '<span></span>',
                        className: [styles.counter],
                        children: [
                            UltraComponent({
                                component:
                                    '<input type="text" inputmode="numeric" pattern="[0-9]*" spellcheck="false" />',
                                className: [styles.counterInput],
                                attributes: { 'aria-label': 'Current page' },
                                eventHandler: {
                                    focus: (evt: Event) =>
                                        (evt.target as HTMLInputElement).select(),
                                    keydown: onCounterKeyDown,
                                    blur: onCounterBlur,
                                },
                                onMount: [onCounterInputChange],
                                trigger: [
                                    {
                                        subscriber: subsCurrentPage,
                                        triggerFunction: onCounterInputChange,
                                    },
                                    {
                                        subscriber: subsPages,
                                        triggerFunction: onCounterInputChange,
                                    },
                                ],
                            }),
                            UltraComponent({
                                component: '<span></span>',
                                onMount: [onCounterTotalChange],
                                trigger: [
                                    {
                                        subscriber: subsPages,
                                        triggerFunction: onCounterTotalChange,
                                    },
                                ],
                            }),
                        ],
                    }),
                    UltraComponent({
                        component: '<button type="button"></button>',
                        className: [styles.refresh],
                        attributes: { 'aria-label': 'Refresh scan' },
                        eventHandler: { click: onRefresh },
                        onMount: [onRefreshingChange],
                        children: [RefreshIcon({ size: 16 })],
                        trigger: [
                            { subscriber: subsIsRefreshing, triggerFunction: onRefreshingChange },
                        ],
                    }),
                ],
            }),
            UltraComponent({
                component: '<div></div>',
                className: [styles.windowControls],
                children: [WindowControls()],
            }),
        ],
    });
}
