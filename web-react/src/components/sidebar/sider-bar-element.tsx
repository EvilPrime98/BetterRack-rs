import { NavLink } from 'react-router-dom';
import { FolderIcon } from '@/icons/folder.icon';
import { useSidebarStore } from '@/stores/sidebar.store';
import type { ILibraryResponseItem } from '@/library.types';
import styles from './sidebar.module.css';
import { useLibraryStore } from '@/stores/library.store';

export function SideBarElement({ item }: { item: ILibraryResponseItem }) {
    const setSearchQuery = useLibraryStore((s) => s.setSearchQuery);
    const setIsExpanded = useSidebarStore((s) => s.setIsExpanded);

    const onClick = () => {
        setSearchQuery('');
        setIsExpanded(false);
    };

    return (
        <NavLink
            to={`/${item.uid}`}
            end
            className={({ isActive }) =>
                [styles.item, isActive ? styles.itemActive : ''].filter(Boolean).join(' ')
            }
            onClick={onClick}
        >
            <FolderIcon size={16} />
            <span>{item.name}</span>
        </NavLink>
    );
}
