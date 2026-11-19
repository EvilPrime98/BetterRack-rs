import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import styles from './header.module.css';
import { useSidebarStore } from '@/stores/sidebar.store';
import { useIsDesktop } from '@/hooks/useIsDesktop';
import { BurgerIcon } from '@/icons/burger-icon';
import { BetterRackIcon } from '@/icons/better-rack.icon';
import { useLibraryStore } from '@/stores/library.store';

export function Header() {
    const iconSize = 24;
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const isDesktop = useIsDesktop();
    const isExpanded = useSidebarStore((s) => s.isExpanded);
    const setIsExpanded = useSidebarStore((s) => s.setIsExpanded);
    const isSideBarCollapsed = useSidebarStore((s) => s.isCollapsed);
    const setIsCollapsed = useSidebarStore((s) => s.setIsCollapsed);
    const isPinned = useSidebarStore((s) => s.isPinned);
    const setSearchQuery = useLibraryStore((s) => s.setSearchQuery);

    function toggleSidebar() {
        if (isDesktop && isPinned) {
            setIsCollapsed(!isSideBarCollapsed);
        } else {
            setIsExpanded(!isExpanded);
        }
    }

    function goHome() {
        setSearchQuery('');
        if (searchParams.get('search')) navigate('/');
    }

    return (
        <header className={styles.header}>
            <button
                className={[styles.iconBtn, styles.burger].filter(Boolean).join(' ')}
                type="button"
                aria-label="Toggle sidebar"
                aria-hidden={false}
                onClick={toggleSidebar}
            >
                <BurgerIcon size={iconSize * 1.5} />
            </button>

            <div className={styles.inner}>
                <div className={styles.left}>
                    <Link
                        to="/"
                        aria-label="BetterRack home"
                        className={[styles.logo].filter(Boolean).join(' ')}
                        onClick={goHome}
                    >
                        <BetterRackIcon size={iconSize * 1.3} />
                    </Link>

                    <div className={styles.text} style={{ userSelect: 'none' }}>
                        <span className={styles.title} style={{ fontSize: '1.15rem' }}>
                            BetterRack
                        </span>
                    </div>
                </div>
            </div>
        </header>
    );
}
