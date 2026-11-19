import type { TJobStatus } from './services/store.service';

export interface IStorePost {
    id?: number;
    thumbnailUrl?: string;
    title: string;
    link: string;
    uploadDate?: string;
}

export interface IStoreBookmark {
    key: string;
    id?: number | null;
    title: string;
    thumbnailUrl?: string | null;
    link: string;
    uploadDate?: string | null;
    folder?: string | null;
    rating: number;
    read: boolean;
    createdAt: number;
}

export interface IPullEntry {
    id: string;
    keyword: string;
    lastSeenId: number;
    createdAt: number;
}

export interface IStoreLink {
    uuid: string;
    title: string;
}

export type TStoreStrat = 'all' | 'single' | 'multiple';

export type TStoreProgressEvent =
    | { type: 'preparing'; title: string }
    | {
          type: 'retrying';
          title: string;
          status?: number;
          reason?: 'network' | 'http';
          delaySec: number;
      }
    | { type: 'progress'; title: string; percent: number; receivedMB: string; totalMB: string }
    | { type: 'extracting'; title: string; done: number; total: number }
    | { type: 'done'; filename: string }
    | { type: 'error'; message: string };

export type TCardState =
    { status: 'idle' } | { status: 'links-loading' } | { status: 'error'; message: string };

export const STRAT = 'all';

export type TJobStates = Map<string, TJobStatus['state']>;
