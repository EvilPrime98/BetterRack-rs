import { UltraComponent, ultraNavigate } from 'ultra-light-js';
import { BRDropdown, type IBRDropdownOption } from '@/components/br-dropdown/br-dropdown';
import { ItemCounter } from '@/components/item-counter/item-counter';
import { StateFilter } from '@/components/state-filter/state-filter';
import { LayoutSelector } from '@/components/layout/layout-selector';
import { BRButton } from '@/components/br-button/br-button';
import { FolderIcon } from '@/icons/folder.icon';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import styles from './page-header.module.css';
import { FILTER_OPTIONS, type ILibraryResponseItem, type ILibraryFilters } from '@/library.types';
import { NEW_FOLDER_MODAL_CTX } from '@/context/new-folder-modal.context';
import { LIBRARY_CONTEXT } from '@/context/library.context';

type TSortOption = 'alphabetical' | 'releaseDate';

const SORT_OPTIONS: IBRDropdownOption<TSortOption>[] = [
    { value: 'alphabetical', label: FILTER_OPTIONS.nofilters },
    { value: 'releaseDate', label: FILTER_OPTIONS.byReleaseDate },
];

export function PageHeader({
    uid,
    items,
    subsItems,
    filters,
    resetFilters,
    showNewFolder = false,
}: {
    uid?: string;
    items: () => ILibraryResponseItem[];
    subsItems: (fn: (value: ILibraryResponseItem[]) => void) => () => void;
    filters: ILibraryFilters;
    resetFilters: () => void;
    showNewFolder?: boolean;
}) {
    const getTitle = () =>
        LIBRARY_CONTEXT.groups
            .get()
            .flatMap((g) => g.entries)
            .find((e) => e.uid === uid)?.name || 'Root';

    return UltraComponent({
        component: '<header></header>',
        className: [styles.pageHeader],

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.filtersRow],
                children: [
                    UltraComponent({
                        component: '<div></div>',
                        className: [styles.left],
                        children: [
                            ...(uid
                                ? [
                                      BRButton({
                                          text: '',
                                          variant: 'secondary',
                                          className: [styles.backButton],
                                          attributes: { 'aria-label': 'Back to library' },
                                          eventHandler: {
                                              click: () => ultraNavigate({ href: '/' }),
                                          },
                                          children: [ArrowLeftIcon({ size: 16 })],
                                      }),
                                  ]
                                : []),
                            BRDropdown<TSortOption>({
                                className: [styles.sortDropdown],
                                options: SORT_OPTIONS,
                                triggerLabel: getTitle,
                                value: () =>
                                    filters.sortByReleaseDate.get()
                                        ? 'releaseDate'
                                        : 'alphabetical',
                                subscribe: (fn) => {
                                    const unsubFilter = filters.sortByReleaseDate.subscribe(fn);
                                    const unsubGroups = LIBRARY_CONTEXT.groups.subscribe(fn);
                                    return () => {
                                        unsubFilter();
                                        unsubGroups();
                                    };
                                },
                                onChange: (value) => {
                                    if (value === 'releaseDate')
                                        filters.sortByReleaseDate.set(true);
                                    else resetFilters();
                                },
                                ariaLabel: () => `Sort options, currently ${getTitle()}`,
                            }),
                            ItemCounter({ items, subsItems }),
                        ],
                    }),

                    UltraComponent({
                        component: '<div></div>',
                        className: [styles.right],
                        children: [
                            StateFilter(),
                            LayoutSelector(),
                            ...(showNewFolder
                                ? [
                                      UltraComponent({
                                          component: '<span></span>',
                                          className: [styles.divider],
                                          attributes: { 'aria-hidden': 'true' },
                                      }),
                                      BRButton({
                                          text: 'New Folder',
                                          variant: 'secondary',
                                          className: [styles.newFolderButton],
                                          eventHandler: {
                                              click: () =>
                                                  NEW_FOLDER_MODAL_CTX.openNewFolderModal(uid),
                                          },
                                          children: [FolderIcon({ size: 14 })],
                                      }),
                                  ]
                                : []),
                        ],
                    }),
                ],
            }),
        ],
    });
}
