import { UltraActivity, UltraComponent, ultraState } from 'ultra-light-js';
import styles from './new-folder-modal.module.css';
import { BRButton } from '@/components/br-button/br-button';
import { NEW_FOLDER_MODAL_CTX } from '@/context/new-folder-modal.context';

export function NewFolderModal() {
    const [text, setText] = ultraState('');

    const cancel = () => NEW_FOLDER_MODAL_CTX.closeNewFolderModal();

    const submit = () => {
        NEW_FOLDER_MODAL_CTX.submitNewFolder(text());
    };

    const onKeydown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') cancel();
        if (e.key === 'Enter') submit();
    };

    const onVisibleChange = ($input: HTMLElement) => {
        if (!NEW_FOLDER_MODAL_CTX.isVisible.get() || !$input) return;
        ($input as HTMLInputElement).value = '';
        $input.focus();
    };

    const onMount = () => {
        document.addEventListener('keydown', onKeydown);
        return () => document.removeEventListener('keydown', onKeydown);
    };

    return UltraActivity({
        mode: {
            state: NEW_FOLDER_MODAL_CTX.isVisible.get,
            subscriber: NEW_FOLDER_MODAL_CTX.isVisible.subscribe,
        },

        component: '<div></div>',

        className: [styles.overlay],

        onMount: [onMount],

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
                    'aria-label': 'Create folder',
                },

                className: [styles.modal],

                children: [
                    `<p class="${styles.title}">New folder</p>`,

                    UltraComponent({
                        component: '<input/>',
                        className: [styles.field],
                        attributes: {
                            type: 'text',
                            placeholder: 'Folder name',
                        },
                        eventHandler: {
                            input: (e) => setText((e.currentTarget as HTMLInputElement).value),
                        },
                        trigger: [
                            {
                                subscriber: NEW_FOLDER_MODAL_CTX.isVisible.subscribe,
                                triggerFunction: onVisibleChange,
                                defer: true,
                            },
                        ],
                    }),

                    UltraComponent({
                        component: '<div></div>',
                        className: [styles.actions],
                        children: [
                            BRButton({
                                text: 'Cancel',
                                variant: 'secondary',
                                eventHandler: { click: cancel },
                            }),
                            BRButton({
                                text: 'Create',
                                variant: 'classic',
                                eventHandler: { click: submit },
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}
