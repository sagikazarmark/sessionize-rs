# Fixtures

Responses from the Sessionize API, one directory per event, one file per view.
Files are pretty-printed with `jq --indent 2 .`; the content is otherwise unchanged.

| Directory | Endpoint | Captured | Notes |
|---|---|---|---|
| `kcd-budapest-2025` | `dpesp6nz` | 2026-10-16 | Real event. Local times, no categories, speaker questions only. GridSmart includes service sessions that `All` doesn't. |
| `kcd-budapest-2025-custom-fields` | `dpesp6nz` | 2026-10-17 | Same event with test custom fields: one session answer per question type and two categories on session `886030`, and a speaker file on Viktor Nagy. |
| `sessionize-demo` | `jl4ktls0` | 2026-10-16 | Sessionize demo event from the API docs. UTC times, categories, links, a service session in `All`. |

To refresh a directory:

```sh
for v in All GridSmart Sessions Speakers SpeakerWall; do
  curl -s "https://sessionize.com/api/v2/$ENDPOINT/view/$v" | jq --indent 2 . > "$v.json"
done
```

The SpeakerWall order is random on every request, so tests must not depend on the order of any view.
