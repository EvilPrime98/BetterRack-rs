// One-off recorder for the br-wiki parity tests. Not run by `cargo test`.
//
//   node record_wiki.mjs <path to BetterRack repo>
//
// Starts one local proxy per wiki that forwards `api.php` requests to the real Fandom wiki
// (one at a time, 150 ms apart), runs the real `better-wiki` plugins through it with exactly
// the options of `WikiModel` (wikiModel.ts), and writes:
//   wiki_recordings.json  every api.php response, per wiki, keyed by query string
//   wiki_golden.json      what better-wiki returned for each scenario (`{WIKI}` = the base URL)
// The Rust tests replay the recordings from a mock server and must produce the same JSON.
import http from 'node:http';
import { writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { resolve, join } from 'node:path';

const root = resolve(process.argv[2]);
const { wiki } = await import(pathToFileURL(join(root, 'node_modules/better-wiki/dist/index.js')).href);

const UA = 'better-wiki (https://www.npmjs.com/package/better-wiki)';
const UPSTREAM = {
  dc: 'https://dc.fandom.com',
  marvel: 'https://marvel.fandom.com',
  image: 'https://imagecomics.fandom.com',
};
const COMIC_FIELDS = ['cover', 'credits', 'pageId', 'issue', 'title', 'volume', 'releaseDate', 'sourceWiki'];

const recordings = { dc: {}, marvel: {}, image: {} };
let queue = Promise.resolve();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let upstreamCalls = 0;

function startProxy(key) {
  return new Promise((ok) => {
    const server = http.createServer((req, res) => {
      const search = new URL(req.url, 'http://x').search;
      queue = queue.then(async () => {
        try {
          await sleep(150);
          upstreamCalls++;
          const r = await fetch(`${UPSTREAM[key]}/api.php${search}`, { headers: { 'User-Agent': UA } });
          const text = await r.text();
          if (!r.ok) throw new Error(`upstream ${r.status}`);
          recordings[key][search] = JSON.parse(text);
          res.writeHead(200, { 'content-type': 'application/json' }).end(text);
        } catch (e) {
          console.error('proxy error', key, search.slice(0, 120), String(e));
          res.writeHead(502).end('bad gateway');
        }
      });
    });
    server.listen(0, '127.0.0.1', () => ok({ server, url: `http://127.0.0.1:${server.address().port}` }));
  });
}

const proxies = {};
for (const key of Object.keys(UPSTREAM)) proxies[key] = await startProxy(key);

const client = (key) => wiki({ plugin: key === 'marvel' ? 'marvel-fandom' : 'dc-fandom', url: proxies[key].url });
const SIZE = 120;
const ops = {
  single: (key, title) =>
    client(key).getComic(title, key === 'marvel'
      ? { thumbnailSize: SIZE, includeCollections: true }
      : { thumbnailSize: SIZE, includeCollections: true, fields: COMIC_FIELDS }),
  multiple: (key, title) => client(key).getComic(title, { multiple: true, thumbnailSize: SIZE, includeCollections: true }),
  byId: (key, _t, pageId) => client(key).getComicById(pageId, { thumbnailSize: SIZE }),
};

const scenarios = [
  { wiki: 'dc', op: 'single', title: 'Batman 001 (2016).cbz' },
  { wiki: 'dc', op: 'single', title: 'Detective Comics 1000.cbz' },
  { wiki: 'dc', op: 'multiple', title: 'Batman Year One' },
  { wiki: 'marvel', op: 'single', title: 'Immortal Hulk 001 (2018).cbz' },
  { wiki: 'marvel', op: 'single', title: 'Amazing Spider-Man 300.cbz' },
  { wiki: 'image', op: 'single', title: 'Saga 001.cbz' },
  { wiki: 'dc', op: 'single', title: 'zzqx nothing matches this 99.cbz' },
];

const golden = [];
for (const s of scenarios) {
  const entry = { ...s };
  try {
    entry.result = (await ops[s.op](s.wiki, s.title)) ?? null;
  } catch (e) {
    entry.error = String(e);
  }
  golden.push(entry);
  console.log(s.wiki, s.op, JSON.stringify(s.title), entry.error ?? (Array.isArray(entry.result) ? `${entry.result.length} comics` : entry.result ? entry.result.title : 'null'));
}
// by-id lookups for the first hit of two of the single lookups
for (const [i, w] of [[0, 'dc'], [3, 'marvel']]) {
  const pageId = golden[i].result?.pageId;
  if (!pageId) continue;
  const entry = { wiki: w, op: 'byId', pageId };
  try {
    entry.result = (await ops.byId(w, null, pageId)) ?? null;
  } catch (e) {
    entry.error = String(e);
  }
  golden.push(entry);
  console.log(w, 'byId', pageId, entry.error ?? entry.result?.title);
}
// a page id that does not exist
{
  const entry = { wiki: 'dc', op: 'byId', pageId: 999999999 };
  entry.result = (await ops.byId('dc', null, 999999999)) ?? null;
  golden.push(entry);
}

// Normalise the proxy base URLs so the output does not depend on the port.
const scrub = (key, value) => JSON.stringify(value).replaceAll(proxies[key].url, '{WIKI}');
const out = golden.map((g) => JSON.parse(scrub(g.wiki, g)));
const rec = { dc: recordings.dc, marvel: recordings.marvel, image: recordings.image };

writeFileSync(new URL('./wiki_golden.json', import.meta.url), JSON.stringify(out, null, 1));
writeFileSync(new URL('./wiki_recordings.json', import.meta.url), JSON.stringify(rec));
console.log('upstream calls:', upstreamCalls, 'recorded:', Object.values(rec).map((r) => Object.keys(r).length));
for (const p of Object.values(proxies)) p.server.close();
process.exit(0);
