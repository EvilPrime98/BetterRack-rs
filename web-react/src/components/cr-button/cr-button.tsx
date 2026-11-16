import type { ButtonHTMLAttributes } from 'react';
import styles from './cr-button.module.css';

interface CrButtonProps extends ButtonHTMLAttributes<
    HTMLSpanElement extends never ? never : HTMLElement
> {
    text: string;
    variant?: 'cyan' | 'red' | 'orange';
}

export function CrButton({ text, variant = 'cyan', className, ...props }: CrButtonProps) {
    return (
        <span
            className={[className, styles.crButton, styles[variant]].filter(Boolean).join(' ')}
            {...props}
        >
            <span>{text}</span>
        </span>
    );
}
