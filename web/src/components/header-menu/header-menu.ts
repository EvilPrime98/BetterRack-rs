import { UltraComponent, UltraActivity, ultraState } from 'ultra-light-js';
import styles from './header-menu.module.css';
import headerStyles from './header.module.css';
import { MenuIcon } from '../../icons/menu.icon';
import { LIBRARY_CONTEXT } from '../../context/library.context';

function onEnterOrSpace(handler: () => void) {
    return (e: Event) => {
        const key = (e as KeyboardEvent).key;
        if (key !== 'Enter' && key !== ' ') return;
        e.preventDefault();
        handler();
    };
}

export function HeaderMenu() {
    const [isOpen, setOpen, subsOpen] = ultraState(false);

    const closeMenu = () => setOpen(false);

    const toggleMenu = (e: Event) => {
        e.stopPropagation();
        setOpen(!isOpen());
    };

    const onOpenChange = ($root: HTMLElement) => {
        $root.classList.toggle(styles.open, isOpen());
    };

    const refresh = () => {
        LIBRARY_CONTEXT.refreshLibraryWithPrompt();
        setOpen(false);
    };

    return UltraComponent({
        component: '<div></div>',

        className: [styles.menuWrap, headerStyles.noDrag],

        eventHandler: {
            click: toggleMenu,
        },

        onMount: [
            onOpenChange,
            () => {
                document.addEventListener('click', closeMenu);
                return () => document.removeEventListener('click', closeMenu);
            },
        ],

        trigger: [
            {
                subscriber: subsOpen,
                triggerFunction: onOpenChange,
            },
        ],

        children: [
            UltraComponent({
                component: MenuIcon({ size: 18 }),
                className: [headerStyles.iconBtn],
                attributes: {
                    role: 'button',
                    tabindex: '0',
                    'aria-haspopup': 'menu',
                    'aria-label': 'More options',
                },
                eventHandler: {
                    keydown: onEnterOrSpace(() => setOpen(!isOpen())),
                },
            }),

            UltraActivity({
                component: `<ul class="${styles.menu}" role="menu"></ul>`,

                mode: {
                    state: isOpen,
                    subscriber: subsOpen,
                },

                children: [
                    UltraComponent({
                        component: `<li class="${styles.option}">Refresh library</li>`,
                        attributes: {
                            role: 'menuitem',
                            tabindex: '0',
                        },
                        eventHandler: {
                            click: (e: Event) => {
                                e.stopPropagation();
                                refresh();
                            },
                            keydown: onEnterOrSpace(refresh),
                        },
                    }),
                ],
            }),
        ],
    });
}
