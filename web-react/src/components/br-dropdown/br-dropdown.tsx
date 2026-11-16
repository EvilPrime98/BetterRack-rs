import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import styles from './br-dropdown.module.css';

export interface BRDropdownOption<T extends string> {
    value: T;
    label: string;
}

interface BRDropdownProps<T extends string> {
    options: BRDropdownOption<T>[];
    value: T;
    onChange: (value: T) => void;
    id?: string;
    triggerLabel?: string;
    className?: string;
    'aria-label'?: string;
    'aria-labelledby'?: string;
}

export function BRDropdown<T extends string>({
    options,
    value,
    onChange,
    id,
    triggerLabel,
    className,
    ...aria
}: BRDropdownProps<T>) {
    const rootRef = useRef<HTMLDivElement>(null);
    const listId = useId();
    const [isOpen, setIsOpen] = useState(false);
    const [activeIndex, setActiveIndex] = useState(0);

    const selectedIndex = Math.max(
        0,
        options.findIndex((o) => o.value === value),
    );
    const selected = options[selectedIndex];

    useEffect(() => {
        if (!isOpen) return;
        function onPointerDown(event: PointerEvent) {
            if (!rootRef.current?.contains(event.target as Node)) setIsOpen(false);
        }
        document.addEventListener('pointerdown', onPointerDown);
        return () => document.removeEventListener('pointerdown', onPointerDown);
    }, [isOpen]);

    function open() {
        setActiveIndex(selectedIndex);
        setIsOpen(true);
    }

    function select(index: number) {
        onChange(options[index].value);
        setIsOpen(false);
    }

    function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
        switch (event.key) {
            case 'ArrowDown':
                event.preventDefault();
                if (!isOpen) return open();
                setActiveIndex((i) => Math.min(options.length - 1, i + 1));
                break;
            case 'ArrowUp':
                event.preventDefault();
                if (!isOpen) return open();
                setActiveIndex((i) => Math.max(0, i - 1));
                break;
            case 'Home':
                if (!isOpen) return;
                event.preventDefault();
                setActiveIndex(0);
                break;
            case 'End':
                if (!isOpen) return;
                event.preventDefault();
                setActiveIndex(options.length - 1);
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
                setIsOpen(false);
                break;
            case 'Tab':
                setIsOpen(false);
                break;
        }
    }

    return (
        <div ref={rootRef} className={[styles.root, className].filter(Boolean).join(' ')}>
            <button
                id={id}
                type="button"
                className={styles.trigger}
                role="combobox"
                aria-haspopup="listbox"
                aria-expanded={isOpen}
                aria-controls={listId}
                aria-activedescendant={isOpen ? `${listId}-${activeIndex}` : undefined}
                onClick={() => (isOpen ? setIsOpen(false) : open())}
                onKeyDown={onKeyDown}
                {...aria}
            >
                <span className={styles.label}>
                    {triggerLabel === undefined &&
                        options.map((o) => (
                            <span key={o.value} className={styles.sizer} aria-hidden="true">
                                {o.label}
                            </span>
                        ))}
                    <span className={styles.current} title={triggerLabel}>
                        {triggerLabel ?? selected?.label}
                    </span>
                </span>
                <svg
                    className={[styles.chevron, isOpen ? styles.chevronOpen : '']
                        .filter(Boolean)
                        .join(' ')}
                    viewBox="0 0 12 12"
                    fill="none"
                    aria-hidden="true"
                >
                    <path d="M3 4.5 6 7.5 9 4.5" />
                </svg>
            </button>

            {isOpen && (
                <ul id={listId} role="listbox" className={styles.list}>
                    {options.map((option, index) => (
                        <li
                            key={option.value}
                            id={`${listId}-${index}`}
                            ref={
                                index === activeIndex
                                    ? (el) => el?.scrollIntoView({ block: 'nearest' })
                                    : undefined
                            }
                            role="option"
                            aria-selected={option.value === value}
                            className={[
                                styles.option,
                                index === activeIndex ? styles.active : '',
                                option.value === value ? styles.selected : '',
                            ]
                                .filter(Boolean)
                                .join(' ')}
                            onPointerEnter={() => setActiveIndex(index)}
                            onClick={() => select(index)}
                        >
                            {option.label}
                        </li>
                    ))}
                </ul>
            )}
        </div>
    );
}
