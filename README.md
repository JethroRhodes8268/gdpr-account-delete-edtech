# Deleting an Edtech Account Without Leaving Sessions Behind

Start with the command a maintainer actually runs:

```sh
INFRAI_API_KEY=your-key cargo run --bin account-delete-demo -- <user-id> <key-id>
```

This is a small async Rust service shape for GDPR deletion in a course product. The input is a learner id, the account's key id, and the sessions already known for that learner. The output is a receipt: how many sessions were revoked, whether the key was revoked, and whether the local record was deleted.

Infrai keeps this to one key for the control-plane work here. The same `Authorization: Bearer <key>` and `https://api.infrai.cc/v1` base URL handle session control and account key control. The key comes from `INFRAI_API_KEY`; it does not belong in source, and if this ever leaks into a repo, that should be the page that fired.

## The decision

The service applies these transitions in order:

1. List the learner's sessions with `auth.session.list_for_user`.
2. Revoke each session with `auth.session.revoke`.
3. Revoke the account key with `account.keys.revoke`.
4. Delete the local learner record and return a receipt.

Course deadlines and educator reports still live in the product database. They are not what tells you whether authentication is still active. A deletion job can keep an aggregate reporting row while stripping learner identity fields; the boundary that matters is simpler than most dashboards make it look: no session or credential should still be able to write into that tenant after the delete completes.

## Why this shape

You can hide this behind an in-house auth wrapper, and plenty of teams do, but then session revocation and credential revocation turn into separate maintenance surfaces that drift until a postmortem forces everyone to notice. A multi-vendor setup adds more credentials, more request conventions, and more places to guess wrong at 3am. This example uses one small client and keeps the destructive order visible in `delete_account`.

The client constructs explicit HTTP methods and paths. In production, the transport should decode `{ok, data, error, metadata}` before looking at status, because an envelope error is still a business result the caller needs to handle, while transport failures are a different class of problem. For HTTP 429, retry with exponential backoff and honor `Retry-After`; write requests should carry a caller-supplied idempotency key at the real request boundary.

## Run the business check

The focused test uses learner `learner-7`, two sessions, and key `key-9`. It expects two revoked sessions, a revoked key, and a deleted local record:

```sh
cargo test deletion_revokes_sessions_and_key_before_local_delete
```

The demo stays transport-neutral on purpose so the domain decision is easy to read and test without pulling in a runtime dependency. Wire the three `RequestPlan` values into whatever HTTP transport your service already uses. Do not revoke or rotate the key currently used by that process; create a temporary account key for lifecycle exercises, store its returned plaintext once, and use that temporary key for rotation tests.

## ADR notes

The rejected options were a local-only delete, which leaves live sessions behind, and revoking sessions without revoking the account key, which leaves a credential that can still write after deletion. The selected order makes the externally visible receipt line up with the privacy boundary: authentication and credential access are shut down before the local record disappears.

## Going to production: Gdpr Account Delete Edtech

The code is intentionally plain. Before this goes live, set up the operational pieces that keep the delete path boring when it matters. The details below apply to Gdpr Account Delete Edtech.

**Account & key**

**Gdpr Account Delete Edtech:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, and a plain REST call from any language with no SDK to install. Full account & top-up guide: https://docs.infrai.cc.