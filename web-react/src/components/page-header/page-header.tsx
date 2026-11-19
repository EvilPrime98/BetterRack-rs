import { BRDropdown, type BRDropdownOption } from '@/components/br-dropdown/br-dropdown';
import { ItemCounter } from '@/components/item-counter/item-counter';
import { StateFilter } from '@/components/state-filter/state-filter';
import { LayoutSelector } from '@/components/layout/layout-selector';
import { BRButton } from '@/components/br-button/br-button';
import styles from './page-header.module.css';
import { FILTER_OPTIONS, type TFilterOptions } from '@/library.types';
import { useNewFolderModalContext } from '@/context/NewFolderModalContext.hooks';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import { useNavigate } from 'react-router-dom';
import { useLibraryStore } from '@/stores/library.store';
import { useConfirmModalStore } from '@/stores/confirmModal.store';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { flattenCounts } from '@/lib/flatten';
import { markComicsRead } from '@/lib/mark-folder-read';

const SORT_OPTIONS: BRDropdownOption<TFilterOptions>[] = [
    { value: FILTER_OPTIONS.nofilters, label: FILTER_OPTIONS.nofilters },
    { value: FILTER_OPTIONS.byCreation, label: FILTER_OPTIONS.byCreation },
    { value: FILTER_OPTIONS.byReleaseDate, label: FILTER_OPTIONS.byReleaseDate },
];

type TAction = 'mark-read' | 'flatten';

const capitalizeWords = (s: string) =>
    s
        .split(' ')
        .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
        .join(' ');

export function PageHeader({
    uid,
    comicUids,
    hasSubfolders,
    count,
    sort,
    setSort,
}: {
    uid?: string;
    comicUids: string[];
    hasSubfolders: boolean;
    count: number;
    sort: TFilterOptions;
    setSort: (sort: TFilterOptions) => void;
}) {
    const navigate = useNavigate();
    const { openNewFolderModal } = useNewFolderModalContext();
    const groups = useLibraryStore((s) => s.groups);
    const cache = useComicCacheStore((s) => s.cache);
    const unread = comicUids.filter((u) => (cache[u]?.readPer ?? 0) < 100);
    const title = capitalizeWords(
        groups.flatMap((g) => g.entries).find((e) => e.uid === uid)?.name || 'Root',
    );

    const actions: BRDropdownOption<TAction>[] = [
        { value: 'mark-read', label: 'Mark entire folder as read', disabled: unread.length === 0 },
        { value: 'flatten', label: 'Flatten folder', disabled: !hasSubfolders },
    ];

    async function onMarkRead() {
        const n = unread.length;
        const confirmed = await useConfirmModalStore.getState().confirmDialog({
            title: 'Mark entire folder as read?',
            message: `${n} comic${n === 1 ? '' : 's'} in this folder will be marked as read, replacing any reading progress.`,
            confirmLabel: 'Mark as read',
        });
        if (confirmed) markComicsRead(unread);
    }

    async function onFlatten() {
        const { comics, folders } = flattenCounts(groups, uid);
        const confirmed = await useConfirmModalStore.getState().confirmDialog({
            title: 'Flatten folder?',
            message: `${comics} comic${comics === 1 ? '' : 's'} will be moved into this folder and up to ${folders} subfolder${folders === 1 ? '' : 's'} removed once empty.`,
            confirmLabel: 'Flatten',
        });
        if (confirmed) useLibraryStore.getState().flattenFolder(uid);
    }

    return (
        <header className={styles.pageHeader}>
            <div className={styles.filtersRow}>
                <div className={styles.left}>
                    {uid ? (
                        <BRButton
                            className={styles.backButton}
                            text=""
                            variant="secondary"
                            onClick={() => navigate(-1)}
                            aria-label="Back to library"
                        >
                            <ArrowLeftIcon size={16} />
                        </BRButton>
                    ) : null}
                    <h1 className={styles.title} title={title}>
                        {title}
                    </h1>
                    <BRDropdown<TFilterOptions>
                        fit
                        options={SORT_OPTIONS}
                        value={sort}
                        onChange={setSort}
                        aria-label="Sort"
                    />
                    <ItemCounter className={styles.counter} count={count} label="Comics" />
                </div>

                <div className={styles.right}>
                    <BRDropdown<TAction>
                        fit
                        alignRight
                        options={actions}
                        value={'' as TAction}
                        triggerLabel="Actions"
                        onChange={(a) => (a === 'mark-read' ? onMarkRead() : onFlatten())}
                        aria-label="Actions"
                    />
                    <BRButton
                        text="New Folder"
                        variant="secondary"
                        onClick={() => openNewFolderModal(uid)}
                    />
                    <StateFilter />
                    <LayoutSelector />
                </div>
            </div>
        </header>
    );
}
