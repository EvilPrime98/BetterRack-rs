import { BRContextMenu } from '../br-context-menu/br-context-menu';
import type { BRMenuGroup } from '../br-context-menu/br-context-menu';
import { RefreshIcon } from '../../icons/refresh-icon';
import type { IBookmark } from '../../library.types';
import type { TReadingMode } from '../../stores/userPref.store';

const READING_MODES: { value: TReadingMode; label: string }[] = [
    { value: 'vertical', label: 'Vertical' },
    { value: 'horizontal', label: 'Horizontal' },
];

export function ReaderContextMenu({
    currentPage,
    bookmarks,
    isRefreshing,
    readingMode,
    onReadingModeChange,
    goToPage,
    onRefresh,
}: {
    currentPage: number;
    bookmarks: IBookmark[];
    isRefreshing: boolean;
    readingMode: TReadingMode;
    onReadingModeChange: (mode: TReadingMode) => void;
    goToPage: (page: number) => void;
    onRefresh: () => void;
}) {
    const groups: BRMenuGroup[] = [
        {
            id: 'reading-mode',
            heading: 'Reading mode',
            items: READING_MODES.map((mode) => ({
                kind: 'radio',
                label: mode.label,
                checked: mode.value === readingMode,
                onSelect: () => onReadingModeChange(mode.value),
            })),
        },
        ...(bookmarks.length > 0
            ? [
                  {
                      id: 'bookmarks',
                      heading: 'Jump to bookmark',
                      scrollable: true,
                      items: bookmarks.map((bookmark): BRMenuGroup['items'][number] => ({
                          kind: 'radio',
                          label: `${bookmark.label} · p.${bookmark.page}`,
                          title: `${bookmark.label} · p.${bookmark.page}`,
                          checked: bookmark.page === currentPage,
                          onSelect: () => goToPage(bookmark.page),
                      })),
                  },
              ]
            : []),
        {
            id: 'actions',
            items: [
                {
                    kind: 'action',
                    label: 'Refresh scan',
                    icon: <RefreshIcon size={14} />,
                    busy: isRefreshing,
                    onSelect: onRefresh,
                },
            ],
        },
    ];

    return <BRContextMenu groups={groups} aria-label="Reader options" />;
}
