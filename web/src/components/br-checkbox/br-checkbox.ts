import { UltraComponent, type UltraElementProps } from 'ultra-light-js';
import styles from './br-checkbox.module.css';

export function BRCheckbox({
    text,
    checked = false,
    onChange,
    ...props
}: {
    text: string;
    checked?: boolean;
    onChange?: (checked: boolean) => void;
} & UltraElementProps) {
    const { attributes, className, ...rest } = props;

    return UltraComponent({
        ...rest,

        component: '<label></label>',

        className: [...(className ? className : []), styles.root],

        children: [
            UltraComponent({
                component: '<input type="checkbox" />',
                className: [styles.input],
                attributes,
                onMount: [
                    ($el) => {
                        ($el as HTMLInputElement).checked = checked;
                    },
                ],
                eventHandler: {
                    change: (e) => onChange?.((e.currentTarget as HTMLInputElement).checked),
                },
            }),

            `<span class="${styles.box}" aria-hidden="true">
                <svg class="${styles.check}" viewBox="0 0 12 12" fill="none">
                    <path d="M2.5 6.25 4.9 8.6 9.5 3.6" />
                </svg>
            </span>`,

            `<span class="${styles.label}">${text}</span>`,
        ],
    });
}
