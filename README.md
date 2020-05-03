# registry

A registry for packages. Speaks the npm registry protocol. Written to explore writing async rust HTTP servers.

## TODO

- cacache readthrough
- package filter for readable store
- 
- `get_packument` Redis cache: skip if the packument is too big to cache
- `get_packument` Redis cache: add setting for "skip packuments" / "skip tarballs"
- `ReadableStore` gzip hint: it'd be nice to hint to the redis caching layer
  that the incoming request supports gzip so we can return a gzipped packument.
