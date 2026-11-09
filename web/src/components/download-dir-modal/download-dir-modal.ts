import { UltraActivity, UltraComponent } from 'ultra-light-js';
import styles from './download-dir-modal.module.css';
import { FolderIcon } from '@/icons/folder.icon';
import { BRCheckbox } from '@/components/br-checkbox/br-checkbox';
import { SETTINGS_CONTEXT } from '@/context/settings.context';
import { DOWNLOAD_DIR_MODAL_CTX } from '@/context/download-dir-modal.context';
import {
    areDirectoryListsEqual,
    getCachedDirectories,
    refreshDirectories,
} from '@/services/fs.service';

function normalizePath(dir: string): string {
    return dir.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

function getTopLevelDirs(dirs: string[]): string[] {
    const normalized = dirs.map(normalizePath);
    return dirs.filter(
        (_, i) => !normalized.some((other, j) => j !== i && normalized[i].startsWith(`${other}/`)),
    );
}

export function DownloadDirModal() {
    let $list: HTMLElement | null = null;
    let loading = false;
    let displayedDirs: string[] | null = null;
    let query = '';
    let showSubfolders = true;

    const cancel = () => DOWNLOAD_DIR_MODAL_CTX.closeDownloadDirModal();

    const onKeydown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') cancel();
    };

    const renderMessage = (text: string) => {
        if (!$list) return;
        $list.replaceChildren(
            UltraComponent({ component: `<li class="${styles.empty}">${text}</li>` }),
        );
    };

    const onVisibleChange = ($input: HTMLElement) => {
        if (!DOWNLOAD_DIR_MODAL_CTX.isVisible.get() || !$input) return;
        query = '';
        ($input as HTMLInputElement).value = '';
        $input.focus();
        if (displayedDirs) renderList(displayedDirs);
    };

    const renderList = (dirs: string[]) => {
        if (!$list) return;

        const currentDefault = SETTINGS_CONTEXT.settings.get().downloadDir;

        if (dirs.length === 0) {
            renderMessage(
                'No directories available. Set a download or library folder in Settings.',
            );
            return;
        }

        const candidates = showSubfolders ? dirs : getTopLevelDirs(dirs);
        const normalizedQuery = query.trim().toLowerCase();
        const visibleDirs = normalizedQuery
            ? candidates.filter((dir) => dir.toLowerCase().includes(normalizedQuery))
            : candidates;

        if (visibleDirs.length === 0) {
            renderMessage('No folders match your search.');
            return;
        }

        $list.replaceChildren(
            ...visibleDirs.map((dir) => {
                const isDefault = dir === currentDefault;
                return UltraComponent({
                    component: `<li class="${styles.item}${isDefault ? ` ${styles.default}` : ''}" title="${dir}"></li>`,
                    eventHandler: { click: () => DOWNLOAD_DIR_MODAL_CTX.confirmDownloadDir(dir) },
                    children: [
                        FolderIcon({ size: 14, color: isDefault ? '#34c3d1' : '#c7c7c7' }),
                        `<span>${dir}</span>`,
                        isDefault ? `<span class="${styles.badge}">default</span>` : '',
                    ],
                });
            }),
        );
    };

    const loadAndRender = async () => {
        if (!$list || loading) return;
        loading = true;
        displayedDirs = getCachedDirectories();
        if (displayedDirs) renderList(displayedDirs);
        else renderMessage('Loading…');
        try {
            const dirs = await refreshDirectories();
            if (!DOWNLOAD_DIR_MODAL_CTX.isVisible.get()) return;
            if (displayedDirs && areDirectoryListsEqual(displayedDirs, dirs)) return;
            displayedDirs = dirs;
            renderList(dirs);
        } catch (e) {
            if (!displayedDirs)
                renderMessage(e instanceof Error ? e.message : 'Failed to load directories.');
        } finally {
            loading = false;
        }
    };

    return UltraActivity({
        mode: {
            state: DOWNLOAD_DIR_MODAL_CTX.isVisible.get,
            subscriber: DOWNLOAD_DIR_MODAL_CTX.isVisible.subscribe,
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
                subscriber: DOWNLOAD_DIR_MODAL_CTX.isVisible.subscribe,
                triggerFunction: () => {
                    if (DOWNLOAD_DIR_MODAL_CTX.isVisible.get()) loadAndRender();
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
                    'aria-label': 'Choose download directory',
                },

                className: [styles.modal],

                children: [
                    `<p class="${styles.title}">Download to: </p>`,

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
                                        if (displayedDirs) renderList(displayedDirs);
                                    },
                                },
                                trigger: [
                                    {
                                        subscriber: DOWNLOAD_DIR_MODAL_CTX.isVisible.subscribe,
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
                                    if (displayedDirs) renderList(displayedDirs);
                                },
                            }),
                        ],
                    }),

                    UltraComponent({
                        component: `<ul class="${styles.list}"></ul>`,
                        onMount: [
                            ($el) => {
                                $list = $el as HTMLElement;
                                if (DOWNLOAD_DIR_MODAL_CTX.isVisible.get()) loadAndRender();
                            },
                        ],
                    }),
                ],
            }),
        ],
    });
}
