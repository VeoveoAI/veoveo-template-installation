# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# file-viewer as published, for Hub's content domain: @file-viewer/web-full's dist, whose
# renderer chunks only it holds, then every Worker, WASM, font and vendor file from
# file-viewer-copy-assets. The assets-* packs' files are in that set already, byte for byte, but
# their license lists and manifests are not, so they join under their own names. Nothing is
# modified, and each file keeps its notices, among them the AGPL-3.0-only DWG, DWF and DWFx
# engine's and LibreDWG's GPL-3.0-only ones, and the source link each carries. Build context:
# factory/file-viewer. The image holds files only, and Hub mounts it read-only.
FROM node:24-trixie-slim@sha256:54f84b4cf8432b21aaffd5d3ce8d41e12cea0d10d33c09d2f3c9879d49d6b446 AS build
WORKDIR /build
COPY package.json package-lock.json check-assets.mjs ./
RUN npm ci --ignore-scripts --no-audit --no-fund \
    && mkdir /out \
    && cp -a node_modules/@file-viewer/web-full/dist/. /out/ \
    && cp node_modules/@file-viewer/web-full/LICENSE /out/LICENSE \
    && npx --no-install file-viewer-copy-assets /out --no-clean \
    && for pack in cad design drawing; do \
         viewer="node_modules/@file-viewer/assets-${pack}/viewer"; \
         cp -an "${viewer}/." /out/; \
         cp "${viewer}/THIRD_PARTY_LICENSES.assets.json" "/out/THIRD_PARTY_LICENSES.assets-${pack}.json"; \
         cp "${viewer}/file-viewer-asset-pack.json" "/out/file-viewer-asset-pack-${pack}.json"; \
       done \
    && rm -f /out/file-viewer-asset-pack.json \
    && node check-assets.mjs /out /out/file-viewer-asset-pack-cad.json \
         /out/file-viewer-asset-pack-design.json /out/file-viewer-asset-pack-drawing.json

FROM scratch AS runtime
COPY --from=build /out /
