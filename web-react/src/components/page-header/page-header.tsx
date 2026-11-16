import { BRDropdown, type BRDropdownOption } from '@/components/br-dropdown/br-dropdown';
import { ItemCounter } from '@/components/item-counter/item-counter';
import { StateFilter } from '@/components/state-filter/state-filter';
import { LayoutSelector } from '@/components/layout/layout-selector';
//import { Breadcrumbs } from '@/components/breadcrumbs/breadcrumbs';
import { BRButton } from '@/components/br-button/br-button';
import { FolderIcon } from '@/icons/folder.icon';
import styles from './page-header.module.css';
import { FILTER_OPTIONS, type ILibraryResponseItem, type ILibraryFilters } from '@/library.types';
import { useNewFolderModalContext } from '@/context/NewFolderModalContext.hooks';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import { useNavigate } from 'react-router-dom';
import { useLibraryStore } from '@/stores/library.store';
import { useConfirmModalStore } from '@/stores/confirmModal.store';
import { flattenCounts } from '@/lib/flatten';

type TSortOption = 'alphabetical' | 'releaseDate';

const SORT_OPTIONS: BRDropdownOption<TSortOption>[] = [
    { value: 'alphabetical', label: FILTER_OPTIONS.nofilters },
    { value: 'releaseDate', label: FILTER_OPTIONS.byReleaseDate },
];

export function PageHeader({
    uid,
    items,
    filters,
    setFilters,
    resetFilters,
    showNewFolder = false,
}: {
    uid?: string;
    items: ILibraryResponseItem[];
    filters: ILibraryFilters;
    setFilters: (updates: Partial<ILibraryFilters>) => void;
    resetFilters: () => void;
    showNewFolder?: boolean;
}) {
    const navigate = useNavigate();
    const { openNewFolderModal } = useNewFolderModalContext();
    const groups = useLibraryStore((s) => s.groups);
    const title = groups.flatMap((g) => g.entries).find((e) => e.uid === uid)?.name || 'Root';
    const hasSubfolders = items.some((i) => i.did);

    async function onFlattenClick() {
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
            {/* {uid ? <Breadcrumbs uid={uid} /> : null} */}

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
                    <BRDropdown<TSortOption>
                        className={styles.sortDropdown}
                        options={SORT_OPTIONS}
                        triggerLabel={title}
                        value={filters.sortByReleaseDate ? 'releaseDate' : 'alphabetical'}
                        onChange={(value) => {
                            if (value === 'releaseDate') setFilters({ sortByReleaseDate: true });
                            else resetFilters();
                        }}
                        aria-label={`Sort options, currently ${title}`}
                    />
                    <ItemCounter className={styles.counter} items={items} />
                </div>

                <div className={styles.right}>
                    <StateFilter />
                    <LayoutSelector />
                    {showNewFolder ? (
                        <>
                            <span className={styles.divider} aria-hidden="true" />
                            {hasSubfolders && (
                                <BRButton
                                    text="Flatten"
                                    variant="secondary"
                                    onClick={onFlattenClick}
                                />
                            )}
                            <BRButton
                                text="New Folder"
                                variant="secondary"
                                className={styles.newFolderButton}
                                onClick={() => openNewFolderModal(uid)}
                            >
                                <FolderIcon size={14} />
                            </BRButton>
                        </>
                    ) : null}
                </div>
            </div>
        </header>
    );
}
