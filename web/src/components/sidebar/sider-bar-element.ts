import { UltraLink } from 'ultra-light-js';
import { FolderIcon } from '../../icons/folder.icon';
import { SIDEBAR_CONTEXT } from '../../context/sidebar.context';
import type { ILibraryResponseItem } from '../../library.types';
import styles from './sidebar.module.css';
import { LIBRARY_CONTEXT } from '@/context/library.context';

export function SideBarElement({ item }: { item: ILibraryResponseItem }) {
    const onClick = () => {
        LIBRARY_CONTEXT.searchQuery.set('');
        SIDEBAR_CONTEXT.isExpanded.set(false);
    };

    const href = `/${item.uid}`;

    function syncActive($a: HTMLElement) {
        $a.classList.toggle(styles.itemActive, window.location.pathname === href);
    }

    function onActiveMount($a: HTMLElement) {
        const onPop = () => syncActive($a);
        syncActive($a);
        window.addEventListener('popstate', onPop);
        return () => window.removeEventListener('popstate', onPop);
    }

    return UltraLink({
        href,
        className: [styles.item],
        onMount: [onActiveMount],
        children: [FolderIcon({ size: 16 }), `<span>${item.name}</span>`],
        eventHandler: {
            click: onClick,
        },
    });
}
