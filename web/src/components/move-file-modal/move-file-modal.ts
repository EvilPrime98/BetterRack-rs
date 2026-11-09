import { UltraActivity, UltraComponent } from 'ultra-light-js';
import styles from './move-file-modal.module.css';
import { FolderIcon } from '@/icons/folder.icon';
import { BRCheckbox } from '@/components/br-checkbox/br-checkbox';
import { LIBRARY_CONTEXT } from '@/context/library.context';
import { MOVE_FILE_MODAL_CTX } from '@/context/move-file-modal.context';
import type { ILibraryGroup } from '@/library.types';

function buildFolderPath(uid: string, groups: ILibraryGroup[]): string {
    const group = groups.find((g) => g.uid === uid);
    if (group) return group.name;

    const byUid = new Map<string, { uid: string; name: string; parentId?: string }>();
    const ownerGroup = new Map<string, ILibraryGroup>();

    groups.forEach((g) =>
        g.entries.forEach((entry) => {
            byUid.set(entry.uid, entry);
            ownerGroup.set(entry.uid, g);
        }),
    );

    const names: string[] = [];
    let current = byUid.get(uid);

    while (current) {
        names.unshift(current.name);
        if (!current.parentId) {
            const owner = ownerGroup.get(current.uid);
            if (owner) names.unshift(owner.name);
            break;
        }
        current = byUid.get(current.parentId);
    }

    return names.join(' / ');
}

export function MoveFileModal() {
    let $list: HTMLElement | null = null;
    let query = '';
    let showSubfolders = true;

    const cancel = () => MOVE_FILE_MODAL_CTX.closeMoveFileModal();

    const onKeydown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') cancel();
    };

    const onVisibleChange = ($input: HTMLElement) => {
        if (!MOVE_FILE_MODAL_CTX.isVisible.get() || !$input) return;
        query = '';
        ($input as HTMLInputElement).value = '';
        $input.focus();
        renderFolders();
    };

    const renderFolders = () => {
        if (!$list) return;

        const groups = LIBRARY_CONTEXT.groups.get();

        const movedUid = MOVE_FILE_MODAL_CTX.fileUid.get();
        const allItems = LIBRARY_CONTEXT.getLibraryItems({ onlyDir: false });
        const excluded = new Set<string>([movedUid]);
        for (let added = true; added;) {
            added = false;
            for (const item of allItems) {
                if (item.parentId && excluded.has(item.parentId) && !excluded.has(item.uid)) {
                    excluded.add(item.uid);
                    added = true;
                }
            }
        }

        const folders = LIBRARY_CONTEXT.getLibraryItems({ onlyDir: true }).filter(
            (folder) => !excluded.has(folder.uid),
        );

        const destinations = folders
            .filter((folder) => showSubfolders || !folder.parentId)
            .map((folder) => ({ uid: folder.uid, path: buildFolderPath(folder.uid, groups) }));

        const normalizedQuery = query.trim().toLowerCase();
        const filteredDestinations = normalizedQuery
            ? destinations.filter((d) => d.path.toLowerCase().includes(normalizedQuery))
            : destinations;
        const showRoot = !normalizedQuery || 'library root'.includes(normalizedQuery);

        $list.replaceChildren(
            ...(showRoot
                ? [
                      UltraComponent({
                          component: `<li class="${styles.item}" title="Library root"></li>`,
                          eventHandler: {
                              click: () => MOVE_FILE_MODAL_CTX.selectMoveTarget(undefined),
                          },
                          children: [
                              FolderIcon({ size: 14, color: '#34c3d1' }),
                              '<span>Library root</span>',
                          ],
                      }),
                  ]
                : []),

            ...filteredDestinations.map(({ uid, path }) =>
                UltraComponent({
                    component: `<li class="${styles.item}" title="${path}"></li>`,
                    eventHandler: { click: () => MOVE_FILE_MODAL_CTX.selectMoveTarget(uid) },
                    children: [FolderIcon({ size: 14, color: '#c7c7c7' }), `<span>${path}</span>`],
                }),
            ),

            ...(!showRoot && !filteredDestinations.length
                ? [
                      UltraComponent({
                          component: `<li class="${styles.empty}">${folders.length ? 'No folders match your search.' : 'No folders yet.'}</li>`,
                      }),
                  ]
                : []),
        );
    };

    return UltraActivity({
        mode: {
            state: MOVE_FILE_MODAL_CTX.isVisible.get,
            subscriber: MOVE_FILE_MODAL_CTX.isVisible.subscribe,
        },

        component: '<div></div>',

        className: [styles.overlay],

        onMount: [
            () => {
                document.addEventListener('keydown', onKeydown);
                return () => document.removeEventListener('keydown', onKeydown);
            },
        ],

        trigger: [
            {
                subscriber: [
                    MOVE_FILE_MODAL_CTX.isVisible.subscribe,
                    LIBRARY_CONTEXT.groups.subscribe,
                ],
                triggerFunction: () => {
                    if (MOVE_FILE_MODAL_CTX.isVisible.get()) renderFolders();
                },
                defer: true,
            },
        ],

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.backdrop],
                eventHandler: { click: cancel },
            }),

            UltraComponent({
                component: '<div></div>',

                attributes: {
                    role: 'dialog',
                    'aria-modal': 'true',
                    'aria-label': 'Move to another folder',
                },

                className: [styles.modal],

                children: [
                    `<p class="${styles.title}">Move to: </p>`,

                    UltraComponent({
                        component: '<div></div>',

                        className: [styles.searchRow],

                        children: [
                            UltraComponent({
                                component: '<input type="text" />',
                                className: [styles.search],
                                attributes: {
                                    placeholder: 'Search folders...',
                                    'aria-label': 'Search folders',
                                },
                                eventHandler: {
                                    input: (e) => {
                                        query = (e.currentTarget as HTMLInputElement).value;
                                        renderFolders();
                                    },
                                },
                                trigger: [
                                    {
                                        subscriber: MOVE_FILE_MODAL_CTX.isVisible.subscribe,
                                        triggerFunction: onVisibleChange,
                                        defer: true,
                                    },
                                ],
                            }),

                            BRCheckbox({
                                className: [styles.checkbox],
                                text: 'Sub-folders',
                                checked: showSubfolders,
                                onChange: (checked) => {
                                    showSubfolders = checked;
                                    renderFolders();
                                },
                            }),
                        ],
                    }),

                    UltraComponent({
                        component: `<ul class="${styles.list}"></ul>`,
                        onMount: [
                            ($el) => {
                                $list = $el as HTMLElement;
                                renderFolders();
                            },
                        ],
                    }),
                ],
            }),
        ],
    });
}
