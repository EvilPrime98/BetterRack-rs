import { UltraActivity, UltraComponent } from 'ultra-light-js';
import styles from './link-picker-modal.module.css';
import { LINK_PICKER_MODAL_CTX } from '@/context/link-picker-modal.context';

export function LinkPickerModal() {
    let $list: HTMLElement | null = null;

    const cancel = () => LINK_PICKER_MODAL_CTX.closeLinkPickerModal();

    const onKeydown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') cancel();
    };

    const onEnterOrSpace = (handler: () => void) => (e: Event) => {
        const key = (e as KeyboardEvent).key;
        if (key !== 'Enter' && key !== ' ') return;
        e.preventDefault();
        handler();
    };

    const renderList = () => {
        if (!$list) return;

        const links = LINK_PICKER_MODAL_CTX.links.get();

        if (links.length === 0) {
            $list.replaceChildren(
                UltraComponent({
                    component: `<li class="${styles.empty}">No links available.</li>`,
                }),
            );
            return;
        }

        $list.replaceChildren(
            ...links.map((link, i) =>
                UltraComponent({
                    component: `<li class="${styles.item}"></li>`,
                    attributes: { role: 'option', 'aria-selected': 'false', tabindex: '0' },
                    eventHandler: {
                        click: () => LINK_PICKER_MODAL_CTX.pickLink(link),
                        keydown: onEnterOrSpace(() => LINK_PICKER_MODAL_CTX.pickLink(link)),
                    },
                    children: [
                        `<span class="${styles.index}">${i + 1}</span>`,
                        `<span class="${styles.label}" title="${link.title}">${link.title}</span>`,
                    ],
                }),
            ),
        );
    };

    return UltraActivity({
        mode: {
            state: LINK_PICKER_MODAL_CTX.isVisible.get,
            subscriber: LINK_PICKER_MODAL_CTX.isVisible.subscribe,
        },

        component: '<div></div>',

        className: [styles.overlay],

        onMount: [
            () => {
                document.addEventListener('keydown', onKeydown);
                return () => document.removeEventListener('keydown', onKeydown);
            },
        ],

        trigger: [
            {
                subscriber: LINK_PICKER_MODAL_CTX.isVisible.subscribe,
                triggerFunction: () => {
                    if (LINK_PICKER_MODAL_CTX.isVisible.get()) renderList();
                },
                defer: true,
            },
        ],

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
                    'aria-label': 'Choose a download link',
                },

                className: [styles.modal],

                children: [
                    UltraComponent({
                        component: '<div></div>',
                        className: [styles.header],
                        children: [
                            `<p class="${styles.title}">Choose a link</p>`,
                            UltraComponent({
                                component: `<p class="${styles.subtitle}"></p>`,
                                trigger: [
                                    {
                                        subscriber: LINK_PICKER_MODAL_CTX.comicTitle.subscribe,
                                        triggerFunction: ($p: HTMLElement) => {
                                            const title = LINK_PICKER_MODAL_CTX.comicTitle.get();
                                            $p.textContent = title;
                                            $p.setAttribute('title', title);
                                        },
                                    },
                                ],
                            }),
                        ],
                    }),

                    UltraComponent({
                        component: `<ul class="${styles.list}" role="listbox"></ul>`,
                        onMount: [
                            ($el) => {
                                $list = $el as HTMLElement;
                                if (LINK_PICKER_MODAL_CTX.isVisible.get()) renderList();
                            },
                        ],
                    }),
                ],
            }),
        ],
    });
}
