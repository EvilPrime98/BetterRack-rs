import { Notyf } from 'notyf';
import 'notyf/notyf.min.css';

const notyf = new Notyf({
    duration: 2500,
    ripple: false,
    dismissible: true,
    position: { x: 'center', y: 'top' },
    types: [
        {
            type: 'success',
            background: 'var(--bg-panel)',
            icon: false,
            className: 'toast toast-success',
        },
        {
            type: 'error',
            background: 'var(--bg-panel)',
            icon: false,
            className: 'toast toast-error',
            duration: 5000,
        },
    ],
});

export const toast = {
    success: (message: string) => notyf.success(message),
    error: (message: string) => notyf.error(message),
};
