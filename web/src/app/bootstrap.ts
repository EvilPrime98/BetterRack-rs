import { COMIC_CACHE_CONTEXT } from '../context/comic-cache.context';
import { USER_PREF } from '../context/user-pref-cache.context';
import { COMICS_TYPE_CTX } from '../context/comics-types.context';
import { SETTINGS_CONTEXT } from '../context/settings.context';
import { LIBRARY_CONTEXT } from '../context/library.context';
import { BOOKMARKS_CTX } from '../context/bookmarks.context';
import { PULL_LIST_CTX } from '../context/pull-list.context';

function shouldRescan() {
    return (
        SETTINGS_CONTEXT.settings.get().rescanOnStartup &&
        LIBRARY_CONTEXT.groups.get().length > 0
    );
}

export async function bootstrap() {
    await Promise.allSettled([
        LIBRARY_CONTEXT.fetchLibrary(),
        COMIC_CACHE_CONTEXT.init(),
        SETTINGS_CONTEXT.fetchSettings(),
        BOOKMARKS_CTX.load(),
        PULL_LIST_CTX.load(),
    ]);
    if (shouldRescan()) await LIBRARY_CONTEXT.identifyLibrary();
    USER_PREF.init();
    COMICS_TYPE_CTX.init();
}
