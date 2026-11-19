import { UltraComponent, type UltraElementProps } from 'ultra-light-js';
import styles from './br-dropdown.module.css';

export interface IBRDropdownOption<T extends string> {
    value: T;
    label: string;
}

let dropdownCount = 0;

export function BRDropdown<T extends string>({
    options,
    value,
    subscribe,
    onChange,
    id,
    triggerLabel,
    ariaLabelledby,
    ariaLabel,
    ...props
}: {
    options: IBRDropdownOption<T>[];
    value: () => T;
    subscribe: (fn: (value: unknown) => void) => () => void;
    onChange: (value: T) => void;
    id?: string;
    triggerLabel?: () => string;
    ariaLabelledby?: string;
    ariaLabel?: string | (() => string);
} & UltraElementProps) {
    const listId = `br-dropdown-${++dropdownCount}`;

    let isOpen = false;
    let activeIndex = 0;
    let $list: HTMLElement | null = null;

    const selectedIndex = () =>
        Math.max(
            0,
            options.findIndex((o) => o.value === value()),
        );

    const $trigger = UltraComponent({
        component: '<button type="button"></button>',
        className: [styles.trigger],
        attributes: {
            ...(id ? { id } : {}),
            role: 'combobox',
            'aria-haspopup': 'listbox',
            'aria-expanded': 'false',
            'aria-controls': listId,
            ...(ariaLabelledby ? { 'aria-labelledby': ariaLabelledby } : {}),
            ...(ariaLabel
                ? { 'aria-label': typeof ariaLabel === 'function' ? ariaLabel() : ariaLabel }
                : {}),
        },
        children: [
            `<span class="${styles.label}">${triggerLabel ? '' : options.map((o) => `<span class="${styles.sizer}" aria-hidden="true">${o.label}</span>`).join('')}<span class="${styles.current}"></span></span>`,
            `<svg class="${styles.chevron}" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                <path d="M3 4.5 6 7.5 9 4.5" />
            </svg>`,
        ],
        eventHandler: {
            click: () => (isOpen ? close() : open()),
            keydown: (e) => onKeyDown(e as KeyboardEvent),
        },
    });

    const $label = $trigger.querySelector(`.${styles.current}`) as HTMLElement;
    const $chevron = $trigger.querySelector('svg') as SVGElement;

    function syncLabel() {
        if (triggerLabel) {
            $label.textContent = triggerLabel();
            $label.title = triggerLabel();
        } else {
            $label.textContent = options[selectedIndex()]?.label ?? '';
        }
        if (typeof ariaLabel === 'function') $trigger.setAttribute('aria-label', ariaLabel());
    }

    function syncActive() {
        if (!$list) return;
        const selected = selectedIndex();
        [...$list.children].forEach(($option, index) => {
            $option.classList.toggle(styles.active, index === activeIndex);
            $option.classList.toggle(styles.selected, index === selected);
            $option.setAttribute('aria-selected', String(index === selected));
        });
        $trigger.setAttribute('aria-activedescendant', `${listId}-${activeIndex}`);
    }

    function onPointerDown(event: PointerEvent) {
        if (!$root.contains(event.target as Node)) close();
    }

    function open() {
        activeIndex = selectedIndex();
        isOpen = true;
        $trigger.setAttribute('aria-expanded', 'true');
        $chevron.classList.add(styles.chevronOpen);
        $list = UltraComponent({
            component: `<ul id="${listId}" role="listbox"></ul>`,
            className: [styles.list],
            children: options.map((option, index) =>
                UltraComponent({
                    component: `<li id="${listId}-${index}" role="option">${option.label}</li>`,
                    className: [styles.option],
                    eventHandler: {
                        pointerenter: () => moveActive(index),
                        click: () => select(index),
                    },
                }),
            ),
        });
        $root.appendChild($list);
        syncActive();
        document.addEventListener('pointerdown', onPointerDown);
    }

    function close() {
        isOpen = false;
        $trigger.setAttribute('aria-expanded', 'false');
        $trigger.removeAttribute('aria-activedescendant');
        $chevron.classList.remove(styles.chevronOpen);
        $list?.remove();
        $list = null;
        document.removeEventListener('pointerdown', onPointerDown);
    }

    function select(index: number) {
        onChange(options[index].value);
        close();
    }

    function moveActive(next: number) {
        activeIndex = next;
        syncActive();
    }

    function onKeyDown(event: KeyboardEvent) {
        switch (event.key) {
            case 'ArrowDown':
                event.preventDefault();
                if (!isOpen) return open();
                moveActive(Math.min(options.length - 1, activeIndex + 1));
                break;
            case 'ArrowUp':
                event.preventDefault();
                if (!isOpen) return open();
                moveActive(Math.max(0, activeIndex - 1));
                break;
            case 'Home':
                if (!isOpen) return;
                event.preventDefault();
                moveActive(0);
                break;
            case 'End':
                if (!isOpen) return;
                event.preventDefault();
                moveActive(options.length - 1);
                break;
            case 'Enter':
            case ' ':
                event.preventDefault();
                if (isOpen) select(activeIndex);
                else open();
                break;
            case 'Escape':
                if (!isOpen) return;
                event.preventDefault();
                event.stopPropagation();
                close();
                break;
            case 'Tab':
                close();
                break;
        }
    }

    syncLabel();

    const $root = UltraComponent({
        ...props,

        component: '<div></div>',

        className: [styles.root, ...(props.className ? props.className : [])],

        children: [$trigger],

        trigger: [
            ...(props.trigger ? props.trigger : []),
            {
                subscriber: subscribe,
                triggerFunction: () => {
                    syncLabel();
                    syncActive();
                },
            },
        ],

        cleanup: [...(props.cleanup ? props.cleanup : []), close],
    });

    return $root;
}
