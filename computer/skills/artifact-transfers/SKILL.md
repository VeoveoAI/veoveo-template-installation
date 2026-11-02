---
name: artifact-transfers
description: "Upload or download Artifact bytes through Hub's signed transfers, keeping signed URLs private."
---

# Artifact transfers

Use Hub's authorized Artifact operations to publish outputs or request downloads.
Transfer bytes with native `/usr/bin/curl`, preserving the exact grant method,
signed URL and required headers. Do not rewrite/redirect the request or mix Hub
or model authorization into it. Keep signed capabilities private: use a mode 0600
curl config under `~` (`umask 077`), run `/usr/bin/curl --config`, then remove
it; avoid verbose/trace output. Ask Hub for a fresh operation if the grant expires.
Complete any required upload/finalize step; byte transfer alone is not publication.
Record safe operation/Artifact IDs, status, byte counts and hashes, not signed URLs.
