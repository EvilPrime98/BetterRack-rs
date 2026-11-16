import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import styles from './sidebar.module.css';
import { useSidebarStore } from '@/stores/sidebar.store';
import { useLibraryStore } from '@/stores/library.store';
import { useIsDesktop } from '@/hooks/useIsDesktop';
import { SideBarGroup } from './sidebar-group';
import { RefreshLibraryButton } from './refresh-button';
import { SidebarSearch } from './sidebar-search';
import { Footer } from '@/components/footer/footer';
import { PinIcon } from '@/icons/pin.icon';
import { GearIcon } from '@/icons/gear.icon';
import { ShopIcon } from '@/icons/shop.icon';
import { DownloadIcon } from '@/icons/download.icon';
import { BookmarkIcon } from '@/icons/bookmark.icon';
import { BookOpenIcon } from '@/icons/book-open.icon';
import { SeriesList } from './series-list';
import { BRButton } from '@/components/br-button/br-button';
import { BRDropdown, type BRDropdownOption } from '@/components/br-dropdown/br-dropdown';
import type { TLibraryStructure } from '@/services/library.service';

const STRUCTURE_OPTIONS: BRDropdownOption<TLibraryStructure>[] = [
    { value: 'folders', label: 'Folders' },
    { value: 'series', label: 'Series' },
];

function PinButton() {
    const isPinned = useSidebarStore((s) => s.isPinned);
    const setIsPinned = useSidebarStore((s) => s.setIsPinned);
    const label = isPinned ? 'Unpin sidebar' : 'Pin sidebar';

    return (
        <button
            type="button"
            className={styles.button}
            aria-label={label}
            aria-pressed={isPinned}
            title={label}
            onClick={() => setIsPinned(!isPinned)}
        >
            <PinIcon
                size={18}
                filled={isPinned}
                color={isPinned ? '#fff' : '#c7c7c7'}
                rotate={90}
            />
        </button>
    );
}

function LibraryList({ isHidden, isLoading }: { isHidden: boolean; isLoading: boolean }) {
    const groups = useLibraryStore((s) => s.groups);
    const structure = useLibraryStore((s) => s.structure);

    if (!groups.length) {
        return (
            <p className={styles.emptyState}>
                {isLoading ? 'Loading library…' : 'No folders found'}
            </p>
        );
    }
    if (structure === 'series') return <SeriesList groups={groups} isHidden={isHidden} />;
    return groups.map((group) => <SideBarGroup key={group.uid} group={group} />);
}

export function SideBar() {
    const asideRef = useRef<HTMLElement>(null);
    const isExpanded = useSidebarStore((s) => s.isExpanded);
    const setIsExpanded = useSidebarStore((s) => s.setIsExpanded);
    const isCollapsed = useSidebarStore((s) => s.isCollapsed);
    const isCompact = useSidebarStore((s) => s.isCompact);
    const setIsCompact = useSidebarStore((s) => s.setIsCompact);
    const fetchLibrary = useLibraryStore((s) => s.fetchLibrary);
    const structure = useLibraryStore((s) => s.structure);
    const setStructure = useLibraryStore((s) => s.setStructure);
    const [isLoading, setIsLoading] = useState(false);
    const isDesktop = useIsDesktop();

    const isPinned = useSidebarStore((s) => s.isPinned);
    const isDocked = isDesktop && isPinned;
    const isHidden = isDocked ? isCollapsed : !isExpanded;

    function closeSidebar() {
        setIsExpanded(false);
    }

    useEffect(() => {
        setIsLoading(true);
        fetchLibrary().finally(() => setIsLoading(false));
    }, [fetchLibrary]);

    useEffect(() => {
        function onKeydown(event: KeyboardEvent) {
            if (event.key === 'Escape') setIsExpanded(false);
        }
        document.addEventListener('keydown', onKeydown);
        return () => document.removeEventListener('keydown', onKeydown);
    }, [setIsExpanded]);

    useEffect(() => {
        const $aside = asideRef.current;
        if (!$aside) return;
        if (!isHidden) {
            $aside.removeAttribute('inert');
        } else {
            if ($aside.contains(document.activeElement)) {
                (document.activeElement as HTMLElement).blur();
            }
            $aside.setAttribute('inert', '');
        }
    }, [isHidden]);

    return (
        <div>
            <div
                className={[styles.backdrop, !isDocked && isExpanded ? styles.visible : '']
                    .filter(Boolean)
                    .join(' ')}
                onClick={closeSidebar}
                aria-hidden="true"
            />

            <aside
                ref={asideRef}
                role="navigation"
                aria-label="Library folders"
                className={[
                    styles.sideBar,
                    isExpanded ? styles.expanded : '',
                    isDocked ? styles.pinned : '',
                    isCollapsed ? styles.collapsed : '',
                ]
                    .filter(Boolean)
                    .join(' ')}
            >
                <div className={styles.topBar}>
                    <BRButton
                        text={isCompact ? 'Show more' : 'Show less'}
                        variant="ghost"
                        className={styles.toggleButton}
                        aria-expanded={!isCompact}
                        onClick={() => setIsCompact(!isCompact)}
                    />
                    {isDesktop && <PinButton />}
                </div>

                <div
                    className={[styles.fold, isCompact ? styles.folded : '']
                        .filter(Boolean)
                        .join(' ')}
                    inert={isCompact}
                >
                    <div className={styles.foldInner}>
                        <div className={styles.section}>
                            <span id="library-group-by-label" className={styles.sectionTitle}>
                                Group by
                            </span>
                            <BRDropdown
                                className={styles.groupByDropdown}
                                aria-labelledby="library-group-by-label"
                                value={structure}
                                onChange={setStructure}
                                options={STRUCTURE_OPTIONS}
                            />
                        </div>

                        <div className={styles.section}>
                            <span className={styles.sectionTitle}>User</span>
                            <Link to="/settings" className={styles.item} onClick={closeSidebar}>
                                <GearIcon size={16} />
                                <span>Settings</span>
                            </Link>
                        </div>

                        <div className={styles.section}>
                            <span className={styles.sectionTitle}>Store</span>
                            <Link to="/store" className={styles.item} onClick={closeSidebar}>
                                <ShopIcon size={16} />
                                <span>Store</span>
                            </Link>
                            <Link
                                to="/store/pull-list"
                                className={styles.item}
                                onClick={closeSidebar}
                            >
                                <BookmarkIcon size={16} />
                                <span>Pull list</span>
                            </Link>
                            <Link
                                to="/store/downloads"
                                className={styles.item}
                                onClick={closeSidebar}
                            >
                                <DownloadIcon size={16} />
                                <span>Downloads</span>
                            </Link>
                        </div>

                        <div className={styles.section}>
                            <span className={styles.sectionTitle}>Browse</span>
                            <Link to="/new" className={styles.item} onClick={closeSidebar}>
                                <BookmarkIcon size={16} />
                                <span>Recently added</span>
                            </Link>
                            <Link to="/reading" className={styles.item} onClick={closeSidebar}>
                                <BookOpenIcon size={16} />
                                <span>Keep reading</span>
                            </Link>
                        </div>

                        <SidebarSearch />

                        <RefreshLibraryButton />
                    </div>
                </div>

                <nav className={styles.list}>
                    <LibraryList isHidden={isHidden} isLoading={isLoading} />
                </nav>

                <div
                    className={[styles.fold, isCompact ? styles.folded : '']
                        .filter(Boolean)
                        .join(' ')}
                    inert={isCompact}
                >
                    <div className={styles.foldInner}>
                        <Footer />
                    </div>
                </div>
            </aside>
        </div>
    );
}
