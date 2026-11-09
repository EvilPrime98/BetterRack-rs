import { UltraComponent } from 'ultra-light-js';
import { SIDEBAR_CONTEXT } from '../../context/sidebar.context';
import styles from './sidebar.module.css';
import { LIBRARY_CONTEXT } from '../../context/library.context';
import { NEW_FOLDER_MODAL_CTX } from '../../context/new-folder-modal.context';
import { SideBarElement } from './sider-bar-element';
import { FolderIcon } from '../../icons/folder.icon';
import { FolderPlusIcon } from '../../icons/folder-plus.icon';
import { ChevronDownIcon } from '../../icons/chevron.icon';
import type { ILibraryGroup } from '../../library.types';

export function SideBarGroup({ group }: { group: ILibraryGroup }) {
    const isExpanded = () => !!SIDEBAR_CONTEXT.expandedGroups.get()[group.uid];
    const subsExpanded = SIDEBAR_CONTEXT.expandedGroups.subscribe;
    let renderedExpanded: boolean | null = null;
    const listId = `sidebar-group-${group.uid}`;

    function toggle() {
        SIDEBAR_CONTEXT.toggleGroup(group.uid);
    }

    function onNewFolder(e: Event) {
        e.stopPropagation();
        NEW_FOLDER_MODAL_CTX.openNewFolderModal(group.uid);
    }

    function onExpandChange($nav: HTMLElement) {
        if (renderedExpanded === isExpanded()) return;
        renderedExpanded = isExpanded();
        if (renderedExpanded) {
            const entries = LIBRARY_CONTEXT.getLibraryItems({ onlyDir: true, uid: group.uid });
            $nav.replaceChildren(...entries.map((item) => SideBarElement({ item })));
        } else {
            $nav.replaceChildren();
        }
    }

    function onAriaExpandedChange($toggle: HTMLElement) {
        $toggle.setAttribute('aria-expanded', String(isExpanded()));
    }

    function onChevronChange($chevron: HTMLElement) {
        $chevron.classList.toggle(styles.chevronExpanded, isExpanded());
    }

    return UltraComponent({
        component: '<div></div>',
        className: [styles.group],
        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.groupHeader],
                children: [
                    UltraComponent({
                        component: '<button type="button"></button>',
                        className: [styles.groupHeaderToggle],
                        attributes: { 'aria-controls': listId, title: group.name },
                        eventHandler: { click: toggle },
                        onMount: [onAriaExpandedChange],
                        trigger: [
                            {
                                subscriber: subsExpanded,
                                triggerFunction: onAriaExpandedChange,
                            },
                        ],
                        children: [
                            `<span class="${styles.groupHeaderIcon}" aria-hidden="true">${FolderIcon({ size: 16 })}</span>`,
                            `<span class="${styles.groupHeaderLabel}">${group.name}</span>`,
                            UltraComponent({
                                component: '<span aria-hidden="true"></span>',
                                className: [styles.chevron],
                                children: [ChevronDownIcon({ size: 14 })],
                                onMount: [onChevronChange],
                                trigger: [
                                    {
                                        subscriber: subsExpanded,
                                        triggerFunction: onChevronChange,
                                    },
                                ],
                            }),
                        ],
                    }),
                    UltraComponent({
                        component: '<button type="button"></button>',
                        className: [styles.groupHeaderAction],
                        attributes: {
                            'aria-label': `New folder in ${group.name}`,
                            title: `New folder in ${group.name}`,
                        },
                        eventHandler: { click: onNewFolder },
                        children: [
                            `<span aria-hidden="true">${FolderPlusIcon({ size: 14 })}</span>`,
                        ],
                    }),
                ],
            }),

            UltraComponent({
                component: '<div role="group"></div>',
                className: [styles.groupList],
                attributes: { id: listId, 'aria-label': group.name },
                onMount: [onExpandChange],
                trigger: [
                    {
                        subscriber: subsExpanded,
                        triggerFunction: onExpandChange,
                    },
                ],
            }),
        ],
    });
}
