import type { AnchorHTMLAttributes, MouseEvent } from 'react';
import { Browser } from '@capacitor/browser';
import { isAndroidPlatform } from '@/services/server-config.service';

export function ExternalLink({
    href,
    onClick,
    ...rest
}: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
    function handleClick(e: MouseEvent<HTMLAnchorElement>) {
        onClick?.(e);
        if (e.defaultPrevented || !isAndroidPlatform()) return;
        e.preventDefault();
        void Browser.open({ url: href });
    }

    return (
        <a {...rest} href={href} target="_blank" rel="noopener noreferrer" onClick={handleClick} />
    );
}
