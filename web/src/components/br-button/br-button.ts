import { UltraComponent, type UltraElementProps } from 'ultra-light-js';
import styles from './br-button.module.css';

export const BR_BUTTON_VARIANTS = {
    primary: 'primary',
    secondary: 'secondary',
    ghost: 'ghost',
    classic: 'classic',
} as const;

export function BRButton({
    text,
    variant = BR_BUTTON_VARIANTS.classic,
    ...props
}: {
    text: string;
    variant?: keyof typeof BR_BUTTON_VARIANTS;
} & UltraElementProps) {
    return UltraComponent({
        ...props,

        component: `<button type="button">${text}</button>`,

        className: [styles.button, styles[variant], ...(props.className ? props.className : [])],
    });
}
