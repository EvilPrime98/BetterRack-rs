import { CloseIcon } from '@/icons/close.icon';
import { useSettingsStore } from '@/stores/settings.store';
import { useLibraryStore } from '@/stores/library.store';
import { toast } from '@/services/toast.service';
import styles from '@/pages/settings.page.module.css';

export function LibraryFolderRow({
    dir,
    clearFolderError,
    setFolderError,
}: {
    dir: string;
    clearFolderError: () => void;
    setFolderError: (newValue: string) => void;
}) {
    const onClick = () => {
        clearFolderError();
        useSettingsStore
            .getState()
            .removeLibraryFolder(dir)
            .then(() => {
                toast.success('Folder removed');
                useLibraryStore.getState().refreshLibrary({ silent: true });
            })
            .catch((e) => {
                const message =
                    e instanceof Error ? e.message : 'There was an error deleting the library.';
                setFolderError(message);
                toast.error(message);
            });
    };

    return (
        <li className={styles.folderItem}>
            <span className={styles.folderPath}>{dir}</span>

            <button
                type="button"
                className={styles.removeBtn}
                aria-label={`Remove ${dir}`}
                onClick={onClick}
            >
                <CloseIcon size={14} />
            </button>
        </li>
    );
}
