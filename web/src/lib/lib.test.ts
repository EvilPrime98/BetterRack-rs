import { describe, expect, it, vi } from 'vitest';
import { filterByWriter, writerFromSearch, writerHref } from './writer-filter';
import { createShortcutRegistry } from './shortcuts';
import { applyMove, readerMoveFor } from './reader-keys';
import { installLeaveGuard } from './leave-guard';
import { countActiveJobs, createDownloadTracker } from './active-downloads';
import { getTopLevelDirs, visibleDirectories } from './directory-picker';
import type { ILibraryGroup, ILibraryResponseItem } from '../library.types';
import type { IStoreBookmark } from '../store.types';
import { bookmarksInFolder, folderDir, mergeBookmarks, onlyNewComic, postKey } from './bookmarks';
import { keywordFromTitle, matches } from './pull-list';
import { inWeek, parseDay, rangeLabel, weekStart } from './week';

const item = (uid: string, writers: string[], did = false) =>
    ({ uid, did, name: uid, comic: { credits: { writers } } }) as unknown as ILibraryResponseItem;

describe('writer filter', () => {
    it('round-trips the writer through the URL', () => {
        const href = writerHref('Alan Moore & Co');
        expect(href.startsWith('/filters?writer=')).toBe(true);
        expect(writerFromSearch(href.slice(href.indexOf('?')))).toBe('Alan Moore & Co');
        expect(writerFromSearch('')).toBe('');
    });

    it('keeps matching comics, skips folders, and an empty writer means all', () => {
        const items = [item('a', ['X']), item('b', ['Y']), item('c', ['X'], true)];
        expect(filterByWriter(items, 'X').map((i) => i.uid)).toEqual(['a']);
        expect(filterByWriter(items, '').map((i) => i.uid)).toEqual(['a', 'b']);
    });
});

describe('shortcut registry', () => {
    const ev = (key: string, extra: object = {}) => ({
        key,
        shiftKey: false,
        ctrlKey: false,
        altKey: false,
        metaKey: false,
        target: null,
        defaultPrevented: false,
        preventDefault: vi.fn(),
        ...extra,
    });

    it('matches modifiers exactly and prefers the latest registration', () => {
        const reg = createShortcutRegistry();
        const global = vi.fn();
        const page = vi.fn();
        reg.register([{ key: 'Escape', handler: global }]);
        const off = reg.register([{ key: 'Escape', handler: page }]);
        expect(reg.handle(ev('Escape'))).toBe(true);
        expect(page).toHaveBeenCalledOnce();
        expect(global).not.toHaveBeenCalled();
        expect(reg.handle(ev('Escape', { shiftKey: true }))).toBe(false);
        off();
        reg.handle(ev('Escape'));
        expect(global).toHaveBeenCalledOnce();
    });

    it('ignores keys typed into inputs', () => {
        const reg = createShortcutRegistry();
        const handler = vi.fn();
        reg.register([{ key: ' ', handler }]);
        expect(reg.handle(ev(' ', { target: { tagName: 'INPUT' } }))).toBe(false);
        expect(handler).not.toHaveBeenCalled();
    });

    it('can leave the default action alone', () => {
        const reg = createShortcutRegistry();
        reg.register([{ key: 'a', preventDefault: false, handler: () => {} }]);
        const e = ev('a');
        reg.handle(e);
        expect(e.preventDefault).not.toHaveBeenCalled();
    });
});

describe('reader keys', () => {
    const metrics = { offset: 100, viewport: 500, max: 1000 };

    it('maps the desktop keys', () => {
        expect(readerMoveFor(' ', false)).toEqual({ type: 'by', unit: 'viewport', amount: 1 });
        expect(readerMoveFor(' ', true)).toEqual({ type: 'by', unit: 'viewport', amount: -1 });
        expect(readerMoveFor('ArrowDown', false)).toEqual({ type: 'by', unit: 'px', amount: 80 });
        expect(readerMoveFor('x', false)).toBeNull();
    });

    it('scrolls and clamps', () => {
        const move = (k: string, s = false) => applyMove(readerMoveFor(k, s)!, metrics);
        expect(move('PageDown')).toBe(600);
        expect(move('PageUp')).toBe(0);
        expect(move('ArrowUp')).toBe(20);
        expect(move('Home')).toBe(0);
        expect(move('End')).toBe(1000);
        expect(applyMove(readerMoveFor('PageDown', false)!, { ...metrics, offset: 900 })).toBe(
            1000,
        );
    });
});

describe('leave guard', () => {
    function setup(count: number) {
        let listener: ((e: BeforeUnloadEvent) => void) | undefined;
        const target = {
            addEventListener: (_: string, fn: (e: BeforeUnloadEvent) => void) => (listener = fn),
            removeEventListener: vi.fn(),
        };
        const remove = installLeaveGuard(target as unknown as Window, () => count);
        const e = { preventDefault: vi.fn(), returnValue: 'x' } as unknown as BeforeUnloadEvent;
        listener!(e);
        return { e, remove, target };
    }

    it('prompts only while downloads are active', () => {
        expect(setup(2).e.preventDefault).toHaveBeenCalled();
        expect(setup(0).e.preventDefault).not.toHaveBeenCalled();
    });

    it('can be removed', () => {
        const { remove, target } = setup(1);
        remove();
        expect(target.removeEventListener).toHaveBeenCalled();
    });
});

describe('download tracker', () => {
    it('counts queued and running jobs', () => {
        expect(
            countActiveJobs([
                { state: 'queued' },
                { state: 'running' },
                { state: 'done' },
                { state: 'error' },
            ]),
        ).toBe(2);
    });

    it('counts a started download at once and polls until it finishes', async () => {
        vi.useFakeTimers();
        const fetchJobs = vi
            .fn()
            .mockResolvedValueOnce([{ state: 'running' }])
            .mockResolvedValueOnce([{ state: 'done' }]);
        const tracker = createDownloadTracker(fetchJobs, 1000);
        tracker.started();
        expect(tracker.count()).toBe(1);
        await vi.advanceTimersByTimeAsync(1000);
        expect(tracker.count()).toBe(1);
        await vi.advanceTimersByTimeAsync(1000);
        expect(tracker.count()).toBe(0);
        tracker.stop();
        vi.useRealTimers();
    });
});

describe('directory picker', () => {
    const dirs = ['C:\\Comics', 'C:\\Comics\\Marvel', 'D:\\Other'];

    it('hides sub-folders on request', () => {
        expect(getTopLevelDirs(dirs)).toEqual(['C:\\Comics', 'D:\\Other']);
    });

    it('filters by query', () => {
        expect(visibleDirectories(dirs, { query: ' marvel ', showSubfolders: true })).toEqual([
            'C:\\Comics\\Marvel',
        ]);
        expect(visibleDirectories(dirs, { query: 'marvel', showSubfolders: false })).toEqual([]);
    });
});

describe('store bookmarks', () => {
    const entry = (uid: string, did: boolean, name = uid, path = '') =>
        ({ uid, did, name, path, parentId: '', createdAt: 0 }) as ILibraryResponseItem;
    const bm = (key: string, title: string, folder?: string) =>
        ({ key, title, folder, rating: 0, read: false, createdAt: 0, link: '' }) as IStoreBookmark;
    const keys = (cells: ReturnType<typeof mergeBookmarks>) =>
        cells.map((c) => (c.type === 'item' ? c.item.uid : c.bookmark.key));

    it('keys posts by id, falling back to the link', () => {
        expect(postKey({ id: 7, link: 'x' })).toBe('id:7');
        expect(postKey({ link: 'x' })).toBe('link:x');
        expect(postKey({ id: 0, link: 'x' })).toBe('link:x');
    });

    it('selects bookmarks by folder, with no folder meaning the root', () => {
        const all = [bm('a', 'a'), bm('b', 'b', 'f')];
        expect(bookmarksInFolder(all, undefined).map((b) => b.key)).toEqual(['a']);
        expect(bookmarksInFolder(all, 'f').map((b) => b.key)).toEqual(['b']);
    });

    it('slots bookmarks alphabetically among comics, after folders', () => {
        const items = [
            entry('f', true),
            entry('a', false, 'Alpha 1'),
            entry('c', false, 'Charlie 1'),
        ];
        const merged = mergeBookmarks(items, [bm('late', 'Zulu 9'), bm('mid', 'Bravo 2')], false);
        expect(keys(merged)).toEqual(['f', 'a', 'mid', 'c', 'late']);
    });

    it('puts bookmarks last when sorted by release date', () => {
        const merged = mergeBookmarks([entry('a', false, 'Alpha')], [bm('b', 'Aaa')], true);
        expect(keys(merged)).toEqual(['a', 'b']);
    });

    it('resolves folder directories', () => {
        const groups: ILibraryGroup[] = [
            { uid: 'g', name: 'Root', path: '/root', entries: [entry('f', true, 'f', '/root/f')] },
        ];
        expect(folderDir(groups, 'g')).toBe('/root');
        expect(folderDir(groups, 'f')).toBe('/root/f');
        expect(folderDir(groups, 'gone')).toBeUndefined();
        expect(folderDir(groups, undefined)).toBeUndefined();
    });

    it('finds the single new comic only', () => {
        const groups: ILibraryGroup[] = [
            { uid: 'g', name: 'R', path: '/r', entries: [entry('old', false)] },
        ];
        const before = new Set(['old']);
        expect(onlyNewComic(groups, before)).toBeUndefined();
        groups[0].entries.push(entry('n1', false));
        expect(onlyNewComic(groups, before)).toBe('n1');
        groups[0].entries.push(entry('n2', false));
        expect(onlyNewComic(groups, before)).toBeUndefined();
    });
});

describe('pull list', () => {
    const entry = (keyword: string, lastSeenId = 0) => ({
        id: 'e',
        keyword,
        lastSeenId,
        createdAt: 0,
    });

    it('matches the case-insensitive series name', () => {
        const e = entry('batman');
        expect(matches(e, 'BATMAN #3')).toBe(true);
        expect(matches(e, 'Batman 012 (2024)')).toBe(true);
        expect(matches(e, 'Batman_Beyond 012 (2024)')).toBe(false);
        expect(matches(e, 'The Dark Knight Batman #3')).toBe(false);
        expect(matches(e, 'Superman #3')).toBe(false);
        expect(matches(entry(' '), 'x')).toBe(false);
    });

    it('guesses a keyword from a title', () => {
        expect(keywordFromTitle('Batman_Beyond 012 (2024)')).toBe('Batman Beyond');
        expect(keywordFromTitle('Saga #54')).toBe('Saga');
        expect(keywordFromTitle('Maus')).toBe('Maus');
    });
});

describe('week', () => {
    const day = (s: string) => parseDay(s) as number;

    it('parses dates and rejects bad ones', () => {
        expect(day('1970-01-01')).toBe(0);
        expect(day('2024-02-29T10:00:00')).toBe(day('2024-02-28') + 1);
        expect(parseDay('2023-02-29')).toBeUndefined();
        expect(parseDay('2023-13-01')).toBeUndefined();
        expect(parseDay('nope')).toBeUndefined();
    });

    it('anchors weeks on Wednesday', () => {
        expect(weekStart(day('2026-11-11'))).toBe(day('2026-11-11'));
        expect(weekStart(day('2026-11-17'))).toBe(day('2026-11-11'));
        expect(weekStart(day('2026-11-18'))).toBe(day('2026-11-18'));
        expect(weekStart(day('2024-12-31'))).toBe(day('2024-12-25'));
        expect(inWeek(day('2026-11-17'), day('2026-11-11'))).toBe(true);
        expect(inWeek(day('2026-11-18'), day('2026-11-11'))).toBe(false);
    });

    it('labels a week range', () => {
        expect(rangeLabel(day('2026-11-11'))).toBe('Nov 11 – Nov 17');
        expect(rangeLabel(day('2024-12-31'))).toBe('Dec 31 – Jan 6');
    });
});
