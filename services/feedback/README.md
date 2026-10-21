# OpenDiag community data service

Receives anonymous results that users choose to share from the app (opt-in, previewed,
confirmed each time) and publishes combined summaries. Cloudflare Worker + D1 database.
What a share contains and why: `docs/privacy.md`, `crates/opendiag-engine/src/share.rs`.

| Address | What |
| --- | --- |
| `POST /v1/shares` | one share → `{ id, delete_token }` |
| `DELETE /v1/shares/:id` | with `Authorization: Bearer <delete_token>` |
| `GET /v1/snapshot` | combined compatibility records (the app's snapshot format, "community reported") |
| `GET /v1/codes` | fault codes per make / model / year |
| `GET /v1/stats` | totals (shares, vehicles, adapters) |

Combinations and codes appear in the summaries only after **2 separate shares** report them.

## Deploy (Cloudflare dashboard, once)

1. **Workers & Pages → Create → Import a repository → MadsBjordal/OpenDiag.**
2. Project name `opendiag-data`; build command empty; deploy command `npx wrangler deploy`;
   **Advanced settings → Root directory `services/feedback`**; preview builds off. Deploy.
3. The D1 database `opendiag-data` is pinned by its ID in `wrangler.jsonc` (a fork with its
   own Cloudflare account removes that `database_id` line once). The first deploy creates it by itself. If the log says it
   cannot (missing permission): **Storage & Databases → D1 → Create database** named
   `opendiag-data`, copy its **Database ID**, add `"database_id": "<id>"` to the `d1_databases`
   entry in `wrangler.jsonc`, commit, and deploy again.
4. **Settings → Domains & Routes → Add → Custom domain:** `opendiag-data.stencilshot.com`
   (the address built into the app). Check: opening `https://opendiag-data.stencilshot.com/v1/stats`
   shows `{"shares":0,…}`.

Updates deploy by themselves when this folder changes on GitHub.

## Looking at the raw data

D1 → `opendiag-data` → **Console**: `SELECT day, data FROM shares ORDER BY day DESC LIMIT 20;`
Remove a share by hand: `DELETE FROM shares WHERE id = '…';`

## Develop and test

```sh
npm test               # rules: what is accepted, how summaries are combined
npm run dev            # local Worker with a local database on http://127.0.0.1:8787
```
End to end with the app's own code: start `npx wrangler dev --port 8799` here, then
`OPENDIAG_DATA_URL=http://127.0.0.1:8799 cargo test -p opendiag-desktop -- --ignored`.
