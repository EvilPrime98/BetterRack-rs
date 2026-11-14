import { UltraRouter } from 'ultra-light-js';
import { SearchPage } from '../pages/search.page';
import { FILTERS_PATH, writerFromSearch } from '../lib/writer-filter';
import { LibraryPage } from '../pages/library-page';
import { ReaderPage } from '../pages/reader.page';
import { SettingsPage } from '../pages/settings.page';
import { StorePage } from '../pages/store.page';
import { StoreDownloadsPage } from '../pages/store-downloads.page';
import { PullListPage } from '../pages/pull-list.page';
import { RecentPage } from '../pages/recent-page';
import { ReadingPage } from '../pages/reading-page';

const READER_SEGMENT = 'reader';
const READER_PATH = `/:uid/${READER_SEGMENT}`;
const READER_PATH_RE = new RegExp(`^/[^/]+/${READER_SEGMENT}/?$`);

export function isReaderPath() {
    return READER_PATH_RE.test(window.location.pathname);
}

export function buildShellRouter() {
    return UltraRouter(
        { path: '/settings', component: () => SettingsPage() },
        { path: '/store/downloads', component: () => StoreDownloadsPage() },
        { path: '/store/pull-list', component: () => PullListPage() },
        { path: '/store', component: () => StorePage() },
        { path: '/new', component: () => RecentPage() },
        { path: '/reading', component: () => ReadingPage() },
        {
            path: FILTERS_PATH,
            component: () =>
                SearchPage({
                    search: '',
                    writer: writerFromSearch(window.location.search),
                }),
        },
        { path: '/:uid', component: ({ uid } = {}) => LibraryPage({ uid }) },
        { path: '/*', component: () => LibraryPage({}) },
    );
}

export function buildReaderRouter() {
    return UltraRouter({
        path: READER_PATH,
        component: ({ uid } = {}) => ReaderPage({ uid }),
    });
}
