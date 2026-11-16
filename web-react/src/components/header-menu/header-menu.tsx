import { useEffect, useState } from 'react';
import styles from './header-menu.module.css';
import headerStyles from '../header/header.module.css';
import { MenuIcon } from '@/icons/menu.icon';
import { useLibraryStore } from '@/stores/library.store';

function onEnterOrSpace(handler: () => void) {
    return (e: React.KeyboardEvent) => {
        const key = e.key;
        if (key !== 'Enter' && key !== ' ') return;
        e.preventDefault();
        handler();
    };
}

export function HeaderMenu() {
    const [isOpen, setOpen] = useState(false);
    const refreshLibrary = useLibraryStore((s) => s.refreshLibraryWithPrompt);

    const closeMenu = () => setOpen(false);

    const toggleMenu = (e: React.MouseEvent) => {
        e.stopPropagation();
        setOpen(!isOpen);
    };

    const refresh = () => {
        refreshLibrary();
        setOpen(false);
    };

    useEffect(() => {
        document.addEventListener('click', closeMenu);
        return () => document.removeEventListener('click', closeMenu);
    }, []);

    return (
        <div className={[styles.menuWrap, isOpen ? styles.open : ''].filter(Boolean).join(' ')}>
            <button
                type="button"
                className={headerStyles.iconBtn}
                aria-haspopup="menu"
                aria-expanded={isOpen}
                aria-label="More options"
                onClick={toggleMenu}
            >
                <MenuIcon size={18} />
            </button>

            {/* UltraActivity: always mounted, visibility toggled via display */}
            <ul
                className={styles.menu}
                role="menu"
                style={{ display: isOpen ? undefined : 'none' }}
            >
                <li
                    className={styles.option}
                    role="menuitem"
                    tabIndex={0}
                    onClick={(e) => {
                        e.stopPropagation();
                        refresh();
                    }}
                    onKeyDown={onEnterOrSpace(refresh)}
                >
                    Refresh library
                </li>
            </ul>
        </div>
    );
}
