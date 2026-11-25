import { UltraActivity, UltraComponent, ultraNavigate, ultraState } from 'ultra-light-js';
import { API_URL, reader, readerBookmarks, readerRefresh } from '../services/library.service';
import { withAuthQuery } from '../services/server-config.service';
import { setImmersiveMode } from '../services/immersive.service';
import type { IBookmark } from '../library.types';
import styles from './reader.page.module.css';
import { ImageElement } from '../components/reader-page-image/reader-page-image';
import { ReaderPageHeader } from '../components/reader-page-header/reader-page-header';
import { ReaderPageProgressBar } from '../components/reader-page-progress-bar/reader-page-progress-bar';
import { COMIC_CACHE_CONTEXT } from '../context/comic-cache.context';
import { DOCUMENT_TITLE_CONTEXT } from '../context/document-title.context';
import { SHORTCUTS } from '../services/shortcuts.service';
import { toggleFullscreen } from '../services/fullscreen.service';
import type { IShortcut } from '../lib/shortcuts';
import { applyMove, readerMoveFor } from '../lib/reader-keys';
import { USER_PREF, type TReadingMode } from '../context/user-pref-cache.context';

const PRELOAD_WINDOW = 2;
const RENDER_BEHIND = 3;
const RENDER_AHEAD = 6;
const ACTIVE_PAGE_SETTLE_MS = 150;

const PREFETCH_AHEAD = 4;
const PREFETCH_BEHIND = 1;

const MIN_ZOOM = 0.5;
const MAX_ZOOM = 3;
const ZOOM_STEP = 0.1;
const PAGE_BASE_WIDTH = 900;
const ZOOM_PERSIST_MS = 300;

const maxZoomFor = (viewerWidth: number) =>
    Math.max(1, Math.floor(Math.min(MAX_ZOOM, viewerWidth / PAGE_BASE_WIDTH) * 100) / 100);

export function ReaderPage({ uid }: { uid: string }) {
    let comicCache = COMIC_CACHE_CONTEXT.getCacheById(uid);
    const [pages, setPages, subsPages] = ultraState<string[]>([]);
    const [title, setTitle, subsTitle] = ultraState('');
    const [isLoading, setIsLoading, subsIsLoading] = ultraState(true);
    const [hasError, setHasError, subsHasError] = ultraState(false);
    const [currentPage, setCurrentPage, subsCurrentPage] = ultraState(comicCache?.currentPage || 1);
    const [bookmarks, setBookmarks, subsBookmarks] = ultraState<IBookmark[]>([]);
    const [isRefreshing, setIsRefreshing, subsIsRefreshing] = ultraState(false);
    const [isHeaderVisible, setIsHeaderVisible, subsIsHeaderVisible] = ultraState(true);
    const [readingMode, setReadingMode, subsReadingMode] = ultraState<TReadingMode>(
        USER_PREF.getPref('readingMode'),
    );
    let observer: IntersectionObserver | null = null;
    let viewer: HTMLElement | null = null;
    let scroller: HTMLElement | null = null;
    let zoomValue = USER_PREF.getPref('zoom');
    let zoomFrame = 0;
    let persistTimer: ReturnType<typeof setTimeout> | undefined;
    let activePage: number | null = null;
    let activeTimer: ReturnType<typeof setTimeout> | undefined;
    let setters: ((active: boolean) => void)[] = [];

    const activePointers = new Map<number, { x: number; y: number }>();
    let pinchStartDistance = 0;
    let pinchStartZoom = 1;
    let isPinching = false;
    let gestureUsedTwoPointers = false;
    let lastTapTime = 0;
    let lastTapX = 0;
    let lastTapY = 0;

    const requested = new Set<number>();

    const warmPages = (numPages: number, pageNumbers: number[]): Promise<void> => {
        const fresh = pageNumbers.filter(
            (page) => page >= 1 && page <= numPages && !requested.has(page),
        );
        if (!fresh.length) return Promise.resolve();
        const loads = fresh.map(
            (page) =>
                new Promise<void>((resolve) => {
                    requested.add(page);
                    const img = new Image();
                    img.onload = img.onerror = () => resolve();
                    img.src = withAuthQuery(`${API_URL}/read/${uid}/pages/${page}`);
                }),
        );
        return Promise.all(loads).then(() => undefined);
    };

    const prefetchAround = (page: number) => {
        const numPages = pages().length;
        if (!numPages) return;
        const targets: number[] = [];
        for (let p = page + 1; p <= page + PREFETCH_AHEAD; ++p) targets.push(p);
        for (let p = page - PREFETCH_BEHIND; p < page; ++p) targets.push(p);
        void warmPages(numPages, targets);
    };

    const goToPage = (page: number) => {
        const $page = viewer?.children[page - 1] as HTMLElement | undefined;
        $page?.scrollIntoView({ block: 'start', inline: 'start' });
        prefetchAround(page);
    };

    const goBack = () => {
        if (window.history.length > 1) window.history.back();
        else ultraNavigate({ href: '/' });
    };

    const getWindowRange = (numPages: number, savedPage: number) => {
        if (numPages < 1) return null;
        const targetInd = Math.min(Math.max(savedPage - 1, 0), numPages - 1);
        const start = Math.max(0, targetInd - PRELOAD_WINDOW);
        const end = Math.min(numPages - 1, targetInd + PRELOAD_WINDOW);
        return { targetInd, start, end };
    };

    const preloadWindow = (numPages: number, savedPage: number): Promise<void> => {
        const range = getWindowRange(numPages, savedPage);
        if (!range) return Promise.resolve();
        const pageNumbers: number[] = [];
        for (let ind = range.start; ind <= range.end; ++ind) pageNumbers.push(ind + 1);
        return warmPages(numPages, pageNumbers);
    };

    const loadPages = async () => {
        setHasError(false);
        setIsLoading(true);
        requested.clear();
        try {
            await COMIC_CACHE_CONTEXT.ready();
            comicCache = COMIC_CACHE_CONTEXT.getCacheById(uid);
            const data = await reader({ uid });
            const savedPage = comicCache?.currentPage || 1;
            await preloadWindow(data.pages.length, savedPage);
            setCurrentPage(savedPage);
            setTitle(data.title);
            setPages(data.pages);
            try {
                setBookmarks(await readerBookmarks({ uid }));
            } catch {
                setBookmarks([]);
            }
        } catch {
            setHasError(true);
        } finally {
            setIsLoading(false);
        }
    };

    const refreshComic = async () => {
        if (isRefreshing()) return;
        setIsRefreshing(true);
        setHasError(false);
        requested.clear();
        try {
            const data = await readerRefresh({ uid });
            setPages(data);
            try {
                setBookmarks(await readerBookmarks({ uid }));
            } catch {
                setBookmarks([]);
            }
        } catch {
            setHasError(true);
        } finally {
            setIsRefreshing(false);
        }
    };

    const flushZoom = () => {
        zoomFrame = 0;
        if (!viewer) return;
        const activePage =
            scroller?.dataset.mode === 'horizontal' && scroller.clientWidth > 0
                ? (viewer.children[Math.round(scroller.scrollLeft / scroller.clientWidth)] as
                      HTMLElement | undefined)
                : undefined;
        const target = activePage ?? scroller;
        const oldHeight = target?.scrollHeight ?? 0;
        const oldWidth = target?.scrollWidth ?? 0;
        const fractionY =
            target && oldHeight > 0
                ? (target.scrollTop + target.clientHeight / 2) / oldHeight
                : null;
        const fractionX =
            activePage && oldWidth > 0
                ? (activePage.scrollLeft + activePage.clientWidth / 2) / oldWidth
                : null;
        viewer.style.setProperty('--reader-zoom', String(zoomValue));
        if (target && fractionY !== null) {
            target.scrollTop = fractionY * target.scrollHeight - target.clientHeight / 2;
        }
        if (activePage && fractionX !== null) {
            activePage.scrollLeft = fractionX * activePage.scrollWidth - activePage.clientWidth / 2;
        }
    };

    const applyZoom = (value: number) => {
        if (value === zoomValue) return;
        zoomValue = value;
        if (!zoomFrame) zoomFrame = requestAnimationFrame(flushZoom);
    };

    const schedulePersistZoom = (value: number) => {
        clearTimeout(persistTimer);
        persistTimer = setTimeout(() => {
            persistTimer = undefined;
            USER_PREF.setPref({ zoom: value });
        }, ZOOM_PERSIST_MS);
    };

    const currentMaxZoom = () => {
        const width = scroller?.clientWidth ?? 0;
        return width > 0 ? maxZoomFor(width) : MAX_ZOOM;
    };

    const clampZoom = (value: number) =>
        Math.min(currentMaxZoom(), Math.max(MIN_ZOOM, +value.toFixed(2)));

    const fitZoom = () => {
        const limit = currentMaxZoom();
        if (zoomValue <= limit) return;
        applyZoom(limit);
        schedulePersistZoom(limit);
    };

    const zoomIn = () => {
        const next = clampZoom(zoomValue + ZOOM_STEP);
        applyZoom(next);
        schedulePersistZoom(next);
    };
    const zoomOut = () => {
        const next = clampZoom(zoomValue - ZOOM_STEP);
        applyZoom(next);
        schedulePersistZoom(next);
    };
    const zoomReset = () => {
        applyZoom(1);
        schedulePersistZoom(1);
    };

    const toggleHeader = () => setIsHeaderVisible(!isHeaderVisible());

    const onHeaderVisibilityChange = ($header: HTMLElement) => {
        $header.classList.toggle(styles.hidden, !isHeaderVisible());
    };

    const onReadingModeChange = (mode: TReadingMode) => {
        setReadingMode(mode);
        USER_PREF.setPref({ readingMode: mode });
    };

    const onReadingModeApply = ($page: HTMLElement) => {
        $page.dataset.mode = readingMode();
        const $current = viewer?.children[currentPage() - 1] as HTMLElement | undefined;
        $current?.scrollIntoView({ block: 'start', inline: 'start' });
    };

    const realign = () => {
        if (readingMode() !== 'horizontal') return;
        const $current = viewer?.children[currentPage() - 1] as HTMLElement | undefined;
        $current?.scrollIntoView({ block: 'nearest', inline: 'start' });
    };

    const pointerGap = () => {
        const points = [...activePointers.values()];
        if (points.length < 2) return 0;
        return Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y);
    };

    const settlePinch = () => {
        if (activePointers.size >= 2) return;
        if (isPinching) schedulePersistZoom(zoomValue);
        isPinching = false;
        pinchStartDistance = 0;
    };

    const onPointerDown = (evt: Event) => {
        const e = evt as PointerEvent;
        if (e.pointerType === 'mouse') return;
        activePointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (activePointers.size === 2) {
            gestureUsedTwoPointers = true;
            isPinching = true;
            pinchStartDistance = pointerGap();
            pinchStartZoom = zoomValue;
        }
    };

    const onPointerMove = (evt: Event) => {
        const e = evt as PointerEvent;
        if (!activePointers.has(e.pointerId)) return;
        activePointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (!isPinching || activePointers.size < 2) return;
        const gap = pointerGap();
        if (pinchStartDistance <= 0 || gap <= 0) return;
        e.preventDefault();
        applyZoom(clampZoom(pinchStartZoom * (gap / pinchStartDistance)));
    };

    const onPointerUp = (evt: Event) => {
        const e = evt as PointerEvent;
        const tracked = activePointers.delete(e.pointerId);
        settlePinch();
        if (tracked && e.pointerType === 'touch' && !gestureUsedTwoPointers) {
            const now = Date.now();
            const quick = now - lastTapTime < 300;
            const close = Math.hypot(e.clientX - lastTapX, e.clientY - lastTapY) < 24;
            if (quick && close) {
                zoomReset();
                lastTapTime = 0;
            } else {
                lastTapTime = now;
                lastTapX = e.clientX;
                lastTapY = e.clientY;
            }
        }
        if (activePointers.size === 0) gestureUsedTwoPointers = false;
    };

    const onPointerCancel = (evt: Event) => {
        const e = evt as PointerEvent;
        activePointers.delete(e.pointerId);
        settlePinch();
        if (activePointers.size === 0) gestureUsedTwoPointers = false;
    };

    const onViewerMount = ($viewer: HTMLElement) => {
        viewer = $viewer;
        scroller = $viewer.closest<HTMLElement>(`.${styles.page}`);
        $viewer.style.setProperty('--reader-zoom', String(zoomValue));
    };

    const onPageRatio = (ratio: number) => {
        if (viewer && !viewer.style.getPropertyValue('--page-ratio')) {
            viewer.style.setProperty('--page-ratio', String(ratio));
        }
    };

    const applyActive = () => {
        if (activePage === null) return;
        const page = activePage;
        setters.forEach((setActive, i) => {
            const n = i + 1;
            setActive(n >= page - RENDER_BEHIND && n <= page + RENDER_AHEAD);
        });
    };

    const requestActivePage = (page: number) => {
        clearTimeout(activeTimer);
        if (activePage === null) {
            activePage = page;
            applyActive();
            return;
        }
        if (page === activePage) return;
        activeTimer = setTimeout(() => {
            activePage = page;
            applyActive();
        }, ACTIVE_PAGE_SETTLE_MS);
    };

    const onHorizontalWheel = (e: WheelEvent) => {
        if (readingMode() !== 'horizontal' || Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
        const $figure = (e.target as HTMLElement).closest('figure');
        const canScrollY =
            $figure &&
            (e.deltaY < 0
                ? $figure.scrollTop > 0
                : $figure.scrollTop + $figure.clientHeight < $figure.scrollHeight - 1);
        if (!canScrollY && scroller) scroller.scrollLeft += e.deltaY;
    };

    const onWheel = (evt: Event) => {
        const e = evt as WheelEvent;
        if (!e.ctrlKey) {
            onHorizontalWheel(e);
            return;
        }
        e.preventDefault();
        if (e.deltaY < 0) zoomIn();
        else if (e.deltaY > 0) zoomOut();
    };

    const onKeydown = (e: KeyboardEvent) => {
        if (!e.ctrlKey) return;
        if (e.code === 'Equal' || e.code === 'NumpadAdd') {
            e.preventDefault();
            zoomIn();
        } else if (e.code === 'Minus' || e.code === 'NumpadSubtract') {
            e.preventDefault();
            zoomOut();
        } else if (e.code === 'Digit0' || e.code === 'Numpad0') {
            e.preventDefault();
            zoomReset();
        }
    };

    const moveBy = (e: KeyboardEvent) => {
        const move = readerMoveFor(e.key, e.shiftKey);
        if (!move || !scroller) return;
        if (readingMode() === 'horizontal') {
            const last = pages().length;
            const target =
                move.type === 'to'
                    ? move.edge === 'start'
                        ? 1
                        : last
                    : currentPage() + (move.amount > 0 ? 1 : -1);
            goToPage(Math.min(last, Math.max(1, target)));
            return;
        }
        scroller.scrollTop = applyMove(move, {
            offset: scroller.scrollTop,
            viewport: scroller.clientHeight,
            max: scroller.scrollHeight - scroller.clientHeight,
        });
    };

    const readerShortcuts = (): IShortcut[] => [
        { key: 'Escape', handler: goBack },
        { key: 'PageDown', handler: moveBy },
        { key: 'PageUp', handler: moveBy },
        { key: ' ', handler: moveBy },
        { key: ' ', shift: true, handler: moveBy },
        { key: 'ArrowDown', handler: moveBy },
        { key: 'ArrowUp', handler: moveBy },
        { key: 'Home', handler: moveBy },
        { key: 'End', handler: moveBy },
    ];

    const onPagesChange = ($section: HTMLElement) => {
        observer?.disconnect();
        viewer = $section;
        setters = [];
        fitZoom();

        const numPages = pages().length;
        const savedPage = comicCache?.currentPage || 1;
        const range = getWindowRange(numPages, savedPage);
        const pageOf = new Map<HTMLElement, number>();
        const elements: HTMLElement[] = [];
        let $target: HTMLElement | null = null;

        for (let i = 0; i < numPages; ++i) {
            const { element: $page, setActive } = ImageElement({
                uid,
                ind: i + 1,
                index: i + 1,
                total: numPages,
                onRatio: onPageRatio,
            });
            pageOf.set($page, i + 1);
            if (range && i === range.targetInd) $target = $page;
            elements.push($page);
            setters.push(setActive);
        }

        $section.replaceChildren(...elements);

        if (numPages) {
            if (activePage === null) requestActivePage(savedPage);
            else {
                applyActive();
                requestActivePage(savedPage);
            }
        }

        $target?.scrollIntoView({ block: 'start', inline: 'start' });

        if (!numPages) return;

        observer = new IntersectionObserver(
            (entries) => {
                const mostVisible = entries
                    .filter((entry) => entry.isIntersecting)
                    .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
                if (!mostVisible) return;
                const page = pageOf.get(mostVisible.target as HTMLElement);
                if (page) {
                    setCurrentPage(page);
                    prefetchAround(page);
                    requestActivePage(page);
                }
            },
            { threshold: [0.25, 0.5, 0.75] },
        );

        elements.forEach(($page) => observer!.observe($page));
    };

    const onProgressChange = () => {
        const total = pages().length || 1;
        const per = (currentPage() / total) * 100;
        COMIC_CACHE_CONTEXT.setCacheById(uid, {
            readPer: Number(per.toFixed(2)),
            currentPage: currentPage(),
            read: per === 100,
        });
    };

    return UltraComponent({
        onMount: [
            loadPages,
            () => DOCUMENT_TITLE_CONTEXT.setTitle('Reader'),
            () => {
                const unregisterShortcuts = SHORTCUTS.register(readerShortcuts());
                window.addEventListener('keydown', onKeydown);
                window.addEventListener('resize', fitZoom);
                window.addEventListener('resize', realign);
                return () => {
                    unregisterShortcuts();
                    window.removeEventListener('keydown', onKeydown);
                    window.removeEventListener('resize', fitZoom);
                    window.removeEventListener('resize', realign);
                    observer?.disconnect();
                    cancelAnimationFrame(zoomFrame);
                    clearTimeout(activeTimer);
                    if (persistTimer) {
                        clearTimeout(persistTimer);
                        USER_PREF.setPref({ zoom: zoomValue });
                    }
                };
            },
            () => {
                setImmersiveMode(true);
                return () => setImmersiveMode(false);
            },
            ($page: HTMLElement) => {
                $page.dataset.mode = readingMode();
            },
            ($page: HTMLElement) => {
                $page.addEventListener('pointerdown', onPointerDown);
                $page.addEventListener('pointermove', onPointerMove, { passive: false });
                $page.addEventListener('pointerup', onPointerUp);
                $page.addEventListener('pointercancel', onPointerCancel);
                return () => {
                    $page.removeEventListener('pointerdown', onPointerDown);
                    $page.removeEventListener('pointermove', onPointerMove);
                    $page.removeEventListener('pointerup', onPointerUp);
                    $page.removeEventListener('pointercancel', onPointerCancel);
                };
            },
        ],

        component: '<section></section>',

        className: [styles.page],

        eventHandler: {
            wheel: onWheel,
        },

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.headerOverlay],
                trigger: [
                    {
                        subscriber: subsIsHeaderVisible,
                        triggerFunction: onHeaderVisibilityChange,
                    },
                ],
                children: [
                    ReaderPageHeader({
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
                        onRefresh: refreshComic,
                    }),

                    ReaderPageProgressBar({
                        currentPage,
                        subsCurrentPage,
                        pages,
                        subsPages,
                    }),
                ],
            }),

            UltraActivity({
                mode: { state: isLoading, subscriber: subsIsLoading },
                component: '<div></div>',
                className: [styles.state],
                children: ['<div class="' + styles.spinner + '"></div>', '<p>Loading pages…</p>'],
            }),

            UltraActivity({
                mode: { state: hasError, subscriber: subsHasError },
                component: '<div></div>',
                className: [styles.state],
                children: [
                    '<p>Something went wrong while loading this comic.</p>',
                    UltraComponent({
                        component: '<button type="button">Retry</button>',
                        className: [styles.retry],
                        eventHandler: { click: loadPages },
                    }),
                ],
            }),

            UltraComponent({
                component: '<section></section>',
                className: [styles.viewer],
                onMount: [onViewerMount],
                eventHandler: {
                    click: toggleHeader,
                    dblclick: toggleFullscreen,
                },
                trigger: [
                    {
                        subscriber: subsPages,
                        triggerFunction: onPagesChange,
                    },
                ],
            }),
        ],

        trigger: [
            {
                subscriber: subsReadingMode,
                triggerFunction: onReadingModeApply,
            },
            {
                subscriber: [subsPages, subsCurrentPage],
                triggerFunction: onProgressChange,
                defer: true,
            },
        ],
    });
}
