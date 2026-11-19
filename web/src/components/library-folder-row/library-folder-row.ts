import { UltraComponent } from 'ultra-light-js';
import { CloseIcon } from '@/icons/close.icon';
import { SETTINGS_CONTEXT } from '@/context/settings.context';
import { LIBRARY_CONTEXT } from '@/context/library.context';
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
        SETTINGS_CONTEXT.removeLibraryFolder(dir)
            .then(() => {
                toast.success('Folder removed');
                LIBRARY_CONTEXT.refreshLibrary({ silent: true });
            })
            .catch((e) => {
                const message =
                    e instanceof Error ? e.message : 'There was an error deleting the library.';
                setFolderError(message);
                toast.error(message);
            });
    };

    return UltraComponent({
        component: '<li></li>',

        className: [styles.folderItem],

        children: [
            UltraComponent({
                component: `<span>${dir}</span>`,
                className: [styles.folderPath],
            }),

            UltraComponent({
                component: '<button></button>',
                className: [styles.removeBtn],
                attributes: {
                    type: 'button',
                    'aria-label': `Remove ${dir}`,
                },
                children: [CloseIcon({ size: 14 })],
                eventHandler: {
                    click: onClick,
                },
            }),
        ],
    });
}
