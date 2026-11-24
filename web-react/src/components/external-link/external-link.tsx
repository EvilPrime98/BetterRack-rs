import type { AnchorHTMLAttributes } from 'react';

export function ExternalLink(props: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
    return <a {...props} target="_blank" rel="noopener noreferrer" />;
}
