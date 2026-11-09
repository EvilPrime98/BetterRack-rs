import type { IUltraCompStateStateful } from 'ultra-light-js';
import type { WikiComic } from 'better-wiki';

export type TMetaSource = 'wiki' | 'comicinfo';

export interface ILibraryItem {
    id: number;
    thumbnail: string;
    readPer: number;
    title: string;
    rating: number;
    issue: string;
    year: string;
}

export interface ILibraryResponseItem {
    uid: string;
    did: boolean;
    name: string;
    path: string;
    parentId: string;
    createdAt: number;
    identified?: boolean;
    comic?: WikiComic;
    metaSource?: TMetaSource;
}

export interface ILibraryGroup {
    uid: string;
    name: string;
    path: string;
    entries: ILibraryResponseItem[];
}

export interface IRecentlyAddedResponse {
    items: ILibraryResponseItem[];
    windowHours: number;
    generatedAt: number;
}

export interface IReadingResponse {
    items: ILibraryResponseItem[];
    generatedAt: number;
}

export const RECENT_WINDOW_OPTIONS = [
    { label: 'Last 24 hours', hours: 24 },
    { label: 'Last week', hours: 168 },
    { label: 'Last month', hours: 720 },
    { label: 'Last year', hours: 8760 },
] as const;

export type TRecentWindow = (typeof RECENT_WINDOW_OPTIONS)[number];

export interface ILibraryIndexGroup {
    uid: string;
    name: string;
    count: number;
}

export interface ILibraryPage {
    groups: ILibraryGroup[];
    total: number;
    limit: number;
    offset: number;
    hasMore: boolean;
}

export interface ILibraryFilters {
    sortByReleaseDate: IUltraCompStateStateful<boolean>;
}

export interface IReadResponse {
    error: boolean;
    message: string;
    title: string;
    pages: string[];
}

export interface IBookmark {
    page: number;
    label: string;
}

export interface IBookmarksResponse {
    error: boolean;
    message: string;
    bookmarks: IBookmark[];
}

export interface ILibraryRefreshResponse {
    error: boolean;
    message: string;
    page?: ILibraryPage;
}

export type TIdentifyProgress =
    | { type: 'identifying'; done: number; total: number }
    | { type: 'done'; total: number }
    | { type: 'error'; message: string };

export type TIdentifyLibraryStatus =
    | { state: 'idle' }
    | {
          jobId: string;
          state: 'queued' | 'running' | 'done' | 'error';
          progress?: TIdentifyProgress;
      };

export interface IComicLSCache {
    rating: number;
    currentPage: number;
    readPer: number;
    read: boolean;
}

export interface IComicIdentity {
    prefId?: number;
    sourceWiki?: string;
    identified?: boolean;
    comic?: WikiComic;
}

export const COMICS_TYPES = {
    cover: 'cover',
    detail: 'detail',
} as const;

export const READ_TYPES = {
    all: 'all',
    read: 'read',
    unread: 'unread',
    reading: 'reading',
} as const;

export type TReadTypes = keyof typeof READ_TYPES;

export type TComicsTypes = keyof typeof COMICS_TYPES;

export const FILTER_OPTIONS = {
    nofilters: 'Alphabetically',
    byCreation: 'Creation Date',
    byReleaseDate: 'Release Date',
} as const;

export type TFilterOptions = (typeof FILTER_OPTIONS)[keyof typeof FILTER_OPTIONS];

export function isIUltraCompStateStateful<T>(
    candidate: object,
): candidate is IUltraCompStateStateful<T> {
    return (
        Object.hasOwn(candidate, 'get') &&
        Object.hasOwn(candidate, 'set') &&
        Object.hasOwn(candidate, 'subscribe')
    );
}
