# HTTP & Networking

Perry natively implements HTTP servers, clients, and WebSocket support.

## Fastify Server

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:fastify-server}}
```

Perry's Fastify implementation is API-compatible with the npm package. Routes, request/reply objects, params, query strings, and JSON body parsing all work.

## Fetch API

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:fetch-api}}
```

## Axios

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:axios-client}}
```

## WebSocket

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:websocket-client}}
```

## Next Steps

- [Databases](database.md)
- [Overview](overview.md) — All stdlib modules
