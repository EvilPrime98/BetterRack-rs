import type { ReactNode } from 'react';
import { Header } from '@/components/header/header';
import { SideBar } from '@/components/sidebar/sidebar';
import { ComicIdentifier } from '@/components/comic-identifier/comic-identifer';
import { ConfirmModal } from '@/components/confirm-modal/confirm-modal';
import { NewFolderModal } from '@/components/new-folder-modal/new-folder-modal';
import { MoveFileModal } from '@/components/move-file-modal/move-file-modal';
import { DownloadDirModal } from '@/components/download-dir-modal/download-dir-modal';
import { LinkPickerModal } from '@/components/link-picker-modal/link-picker-modal';
import { useSidebarStore } from '@/stores/sidebar.store';
import styles from './layout.module.css';

export function Layout({ children }: { children: ReactNode }) {
    const isCollapsed = useSidebarStore((s) => s.isCollapsed);
    const isPinned = useSidebarStore((s) => s.isPinned);

    return (
        <main
            className={[styles.layout, isCollapsed || !isPinned ? styles.sidebarCollapsed : '']
                .filter(Boolean)
                .join(' ')}
        >
            <Header />

            <SideBar />

            <div className={styles.content}>{children}</div>

            <ComicIdentifier />

            <ConfirmModal />

            <NewFolderModal />

            <MoveFileModal />

            <DownloadDirModal />

            <LinkPickerModal />
        </main>
    );
}
