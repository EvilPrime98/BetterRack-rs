import { UltraComponent, type UltraElementProps } from 'ultra-light-js';
import styles from './cr-button.module.css';

export function CrButton({
    text,
    variant = 'cyan',
    ...props
}: {
    text: string;
    variant?: 'cyan' | 'red' | 'orange';
} & UltraElementProps) {
    return UltraComponent({
        ...props,

        component: '<span></span>',

        className: [...(props.className ? props.className : []), styles.crButton, styles[variant]],

        attributes: {
            ...props.attributes,
            type: 'button',
        },

        children: [`<span>${text}</span>`],
    });
}
